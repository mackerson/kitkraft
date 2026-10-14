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
    for padded string tables (done in M3.G2, below).

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
- M3 lane A (container), 2026-10-14:
  - M3.A1 recompression study: the 160 compressed records zlib-rs level 9 does not
    reproduce are listed in `docs/corpus.md` ("Recompression misses";
    `corpus_recompression_misses`) and classified in `docs/gaps.md` ("Recompression
    parity"): 147 NPC_/CELL (class E) differ only in the distance of their last match,
    which runs to the end of the data, so the original compressor's choice depends on
    bytes past the end of the record; 12 NAVM of one cell (class N) are level 8; 1 NAVM is
    unexplained. No zlib-rs window/memory-level/strategy/level/tuning/flush variant, nor C
    zlib 1.3.2 or miniz_oxide, reproduces class E (out-of-tree study, procedure in
    `docs/gaps.md`). Default writer unchanged (level 9) pending M3.H3. F07 candidates filed
    to lane H (Requests).
  - M3.A2 `roundtrip --decode` in the driver: with `RoundtripOptions::decode` every
    compressed body is inflated strictly (declared size, end marker and checksum, nothing
    after the stream), its subrecords parsed and re-encoded to the inflated bytes; a body
    that fails is a `Mismatch` (offset where inflating stopped, path ending in
    `> compressed body: <fault>`), decode limits stay errors. Synthetic fixtures for each
    fault; the M1.7 harness accepts only such decode mismatches. Full corpus 14/14
    identical with it (18.3 s, 1,397 MiB, `docs/corpus.md`). The command side is M3.F1
    (Request to lane F).
- Lanes F and G, M3 (2026-10-14):
  - M3.F2 `coverage` decodes every record of a defined type through its generated
    definition (plugin context: form version, TES4 localized flag) and encodes it again:
    typed occurrences whose payload does not decode are `failed` (counted apart, first 20
    in `typed_failures` with offsets), re-encode differences are `reencode_conflicts`,
    decoded occurrences are split by member confidence (`decoded_by_confidence`).
    `cargo xtask parity [--check]` adds a "Decoded through the definitions" table; corpus:
    20,437 records decoded, 40,776 typed occurrences, 0 failed, 0 re-encode conflicts, all
    `corpus` confidence; `--check` passes on the regenerated file.
  - M3.F3 hygiene: `docs/gaps.md` rows name their closing M3 tasks (the `PLANNED_IN_M2`
    and "MCP server not implemented" rows removed); answered Requests struck through with
    where they were done; `docs/mcp.md` names the coming `refs` (M3.D4).
  - M3.F4: `plan/adr/0007-egui-shell-over-the-catalog.md`, status **Proposed**: options
    (A: minimal egui shell in M3 = M3.F5; B: defer to M4; C: iced, Slint, web), costs, and
    the recommendation B. Awaiting the owner's M3.O6; M3.F5 is gated on it. No UI code.
  - M3.G1 `papyrus.compile` takes `sources_zip` (`kit papyrus compile ... --sources-zip
    <Tools\ContentResources.zip>`): the zip's 5,034 `Scripts/Source` `.psc`/`.flg` files
    (12.3 MB) are extracted once into the KitCraft cache, keyed by name, size and CRC-32,
    hostile names refused before any write, and imported after `imports` (zip crate 9,
    deflate only). PapyrusCompiler 4.7.0.5 with only the zip: DefaultAliasOnActivate,
    DefaultCounterQuestA and the noc patch's nocOutfitChangerQuest,
    nocOutfitChangerContainer, SQ_Airlock_QuestScript, CS7_SpaceshipLandingQuestScript all
    compile (first call 14.5 s with the extraction, then 1-2 s each). Gaps row closed.
  - M3.G2 tolerant string-table parse: `kitcraft_strings::ParseMode::{Strict, Tolerant}`,
    `Table::parse_with` -> `Parsed { table, padding }` (`Padding { kind: DataTail |
    AfterData, offset, len, zeros }`); `Table::parse` stays strict and unchanged.
    `strings.dump` / `strings.merge` take `tolerant` and name each padding run in
    `limitations` and `data.padding`; a strict `DataSize` failure hints at `tolerant`.
    Lane D's string loading (`crates/doc`) still parses strictly.
  - M3.G3 (research) Archive2 1.2.0.1 and non-ASCII names, black-box runs on synthetic
    files (`docs/corpus.md` "Non-ASCII names in Archive2"): names lower-cased and stored
    as Windows-1252 (characters outside it fail the run); name/dir hashes stop at the
    first non-ASCII byte (extension field: 0 there), so such names collide and Archive2
    silently kept 1 of 4; bytes 0xF0..F5/F8..FD hash to run-dependent values (16 of 256
    hashes changed between two runs). `crates/ba2`: names decoded as Windows-1252 (was
    Latin-1; extraction now matches Archive2's names), `hash::archive2_record_hashes`
    with tests from the observed values, the reader's hash check accepts it for non-ASCII
    names. `ba2.pack` keeps refusing them (collisions, unstable bytes, game lookup not
    observed); gaps row restated.
- Lane E, M3 (engine `src/edit/`, `src/validate/`, `src/build/`,
  `handlers/{set,validate,build,overhaul}.rs`, `docs/overhaul/`):
  - M3.E3 `validate` hardening: `FE`/`FD` FormIDs resolve through lane D's class encoding
    (`kitcraft_doc::RawRef`; a master of unknown scale before the index and
    `full-style-self` stay unresolved, never guessed); `scripts` takes `.ba2` archives and
    `plugin_archives` adds the plugin's own `<stem> - *.ba2`. `validate_corpus_official_files`
    (release, 83 s): 0 errors, unresolved FormIDs 0 on all 14 files, warnings unchanged
    (92 `override.stripped-name` on CELL/LCTN/NPC_/QUST/WRLD, the DLCs' own; 15
    `formid.dangling` in SFBGS050, overrides of SFBGS00D forms SFBGS00D lacks).
  - M3.E6 `build` spec v2 (`crates/engine/src/build/`): named classes, `[[rule]]` over a
    class with `via` (typed FormID members followed hop by hop; lane D's `follow`, M3.D4,
    replaces the local one when it lands), operators `mul`/`add`/`min`/`max`/`clamp`,
    one write per record across rules and classes (different writes to one field name both
    keys), `unknown member` checked against the schema, `strict`, `plan` (change list with
    before/after, per-class and per-rule counts, nothing written), and the
    `reference`-confidence gate (`allow_reference`). Built and tested on the members typed
    today (GLOB `FLTV`, GMST `DATA`, `EDID`, `KSIZ`/`KWDA`, `FULL`); the weapon members
    (WEAP/AMMO/PROJ/AMDL, lane B round 3: M3.B1..B3) plug in through the schema with no
    code change. The `reference` gate is tested on a synthetic schema (the builtin has no
    `reference` member yet). Version 1 specs build as before (the rename keeps its hash).
  - M3.E7 `overhaul.scaffold` (CLI `kit overhaul`, `handlers/overhaul.rs`,
    `build/scaffold.rs`): a starter spec v2 per record type, one class per `group_by` value
    with match counts and min/median/max of each typed numeric member as comments, every
    rule commented out; deterministic; `out` guarded (`.toml`, never game/user Data or
    MO2, `overwrite`). On the Default profile `kit overhaul WEAP --group-by KWDA`: 541
    winning WEAP, 40 classes, 0.8 s warm (release); `kit build <scaffold> --plan`: 0
    changes, every class's count equal to the scaffold's (4.5 s). Today the only typed
    numeric WEAP member is `KSIZ`; the weapon stats appear when lane B types WEAP (M3.B1).
  - M3.E8 design brief template `docs/overhaul/brief.md` (source, critique points with
    owner's-words claim, timestamp, classes, desired change, measurable target; out of
    scope; in-game checklist; the traceability rule `brief = "B<n>"`). Next for the owner:
    **M3.O4** (pick the critique, fill the brief), which gates M3.E9.
  - Not started, gated: M3.E1/E2 on lane D's M3.D1 (`missing_active_plugins`,
    `Resolved.bytes`, not on main); M3.E4 on M3.B8; M3.E5 on M3.A3 (and M3.H3/M3.O1);
    M3.E9 on M3.O4 (owner); M3.E10 on M3.B1 (typed WEAP).
- Lane D, M3 (`crates/doc`, `crates/io`, `docs/load-order.md`; engine
  `handlers/{load,inspect,query,diff,conflicts}.rs`), 2026-10-14:
  - M3.D1 lane E's round 2 requests: `Loaded::missing_active_plugins` (a field; also
    `missing_active_plugins` in `load`'s listing) and `kitcraft_doc::source`:
    `LoadOrder::source_check(form, missing_active)` / `Loaded::source_check(form)` returns
    the winner with `blocking` and `unrelated` problems (`is_safe()`); `inspect` shows it as
    `winner_source`. Blocking: no or deleted winner, a version's plugin loading before its
    master, a plugin after the winner that failed to load or has unresolved records that
    could be this form (it names the defining plugin and a stored object ID matches). Default
    profile: the rifle `Starfield.esm:0021BBCD` is safe (unrelated: 4 unresolved records of
    `HG_ShipCargoHoldAtHome.esm`, none can be it). `strings::Resolved` has `bytes` (the
    table's exact bytes) and `utf8`; every non-UTF-8 `starfield_en` entry resolves with its
    bytes (`crates/doc/tests/strings_corpus.rs`).
  - M3.D2 (generic part; proven on today's typed members): `kitcraft_doc::fields` compares
    a subrecord with a schema layout member by member (`typed::decode`), FormIDs resolved
    by plugin name on each side, opaque members as payload byte ranges
    (`Decoded::Opaque { at, len }`); `compare::classify_with` / `diff_subrecords_with`,
    `conflicts::classify_versions`. `diff`, `conflicts` and `inspect`'s `vs_previous` report
    `member` and `fields[] {path, before, after}` (paths `KWDA.Forms[1]`, `FULL.Name`) and
    count `equal_by_value` (bytes differ, values equal: another master order). Corpus:
    SFBGS004.esm vs a copy with one keyword changed reports `KWDA.Forms[0]`
    (`crates/engine/tests/diff_corpus.rs`); differently ordered master lists compare equal.
    WEAP/AMMO/... fields beyond the universal subrecords follow automatically once lane B
    types them (M3.B1+). Default profile `conflicts --count-classes`: 6,029 differs, 1,644
    identical (unchanged by the typed comparison), 2.7 s warm.
  - M3.D3: `kitcraft_doc::confidence::reference_members` finds the `reference`-confidence
    members behind a value (a whole decoded subrecord, or a `where` path); `inspect` and
    `query` name them in `limitations` (`... reference confidence ...: GLOB FLTV.Value`).
    No builtin member is `reference` yet, so the test (`reference_members_surface_as_
    limitations`, through `execute`) installs a schema whose GLOB `FLTV` cites xEdit at
    `reference` (a test-only session slot, `SchemaOverride`). `query` decodes through
    `search::run_page_with(.., schema)` (`run_page` keeps the builtin schema).
  - M3.D6: `crates/doc/tests/typed_corpus.rs` (five small files fast, all 14 `#[ignore]`,
    release 24 s): the interpreter `inspect`/`where`/`diff` decode with agrees with the
    generated decoder behind `Plugin::typed` on all 20,423 records of defined types
    (FormIDs and string IDs), and every lstring ID resolves except 181 pinned corpus facts
    (85 ARMO FULL IDs of Starfield.esm; BlueprintShips-SFBGS050.esm ships no string
    tables). Counts in `docs/corpus.md`; the stale schema/inspect row of `docs/gaps.md`
    closed, the lstring row restated.
  - Not done, gated: M3.D4 (`follow`, `refs`) needs M3.B1 and M3.B2 (typed WEAP, AMMO,
    PROJ) on main; M3.D5 needs the owner sessions M3.O1 (F00) and M3.O2.
- Planet schema family (parallel schema lane, `crates/schema/defs`, 2026-10-14): PCMT,
  PCBN, PCCN, LVLP, PKIN, GBFM, BIOM, PNDT, STDT, SUNP, SFBK, SFPT, RSGD, RSPJ, TRAV
  transcribed from the pinned xEdit files (one commit per type, every member cited with
  its line range); new shared `common/` definitions Condition, ObjectBounds,
  ObjectPaletteDefaults, Transforms, ColorBgra, BiomeNoiseLayer, ResourceBiomeTier,
  ResourceCellTier. Corpus check (14 files, out-of-tree checker over the generated
  decoder): all 26,290 records of the 15 types decode with every subrecord placed, nothing
  in `extra`, and re-encode byte-identically (PNDT after decompression); confidence
  `corpus`, members never seen in an official file `reference`. Opaque by design:
  component blocks, CTDA unions, VMAD, OBTS, NVNM, SUNP REFL/RDIF, unnamed slots (gaps
  rows). PCCN occurs (2,148 records); no RSPG/RSSD exist. Mod-author guide:
  `docs/schema/planet-family.md` (tree composition, pack-ins, the GBFM component
  histogram, opaque members, open questions). ~~Derived files are left to the integrator
  (M4 family lane rule, BX): `crates/esm/src/generated/`, the `BUILTIN_DEFINITIONS` region
  of `crates/schema/src/lib.rs` (23 new entries), `crates/schema/tests/golden/seed.rs`
  (`KITCRAFT_UPDATE_GOLDEN=1`) and the builtin-set lists in `crates/schema/tests/defs.rs`
  (`builtin_set_loads_and_matches_the_directory`).~~ Done in M4.BX1 (PLANET merge,
  2026-10-14): all four regenerated on main (`codegen --check` clean); `cargo xtask ci`
  passed with `KITCRAFT_DATA_DIR`; ignored release corpus tests for esm, doc and engine
  pass (M2.A3 typed check 0 conflicts; doc `typed_corpus` 46,713 typed records, 181
  pinned unresolved lstrings as before); `docs/record-parity.md` regenerated: 25 record
  definitions, 50,419,561 subrecord occurrences, 379,633 typed (0 failed), 218,127
  opaque, 49,821,801 unknown (Starfield.esm parity pass 43.6 s).
- M4.BX1 position-dependent layouts (integrator, 2026-10-14): `kitcraft_doc::typed` now
  resolves a subrecord's layout by its position in the definition (`placements`,
  `layout_of`, `layout_at`, `path_layout`: the generated matcher's rules over the schema
  model, member paths from the definition), never by signature alone; inside an
  `any_until` body or where no member accepts it, the result is a typed `LayoutError`
  (`Raw`, `Unplaced`; `Ambiguous` for a `where` path over occurrences with different
  layouts, `CNAM[1]` picks one) and the payload is opaque. `diff`/`conflicts`
  (`fields::compare`), `where` (`subject`), `inspect`, and lane E's `edit::fields`
  (`layout_at`, `layouts`, replacing the signature-only `layout_for`) with its callers
  (`set`, FormID remap, `inline_lstrings`, `validate`) use it. Regression tests: PKIN VNAM
  inside a component (`pkin_vnam_inside_a_component_is_raw`), PNDT's duplicated
  CNAM/FNAM/GNAM (`pndt_duplicated_signatures_resolve_by_position`,
  `layouts_resolve_by_position`). `validate_corpus_official_files` keeps the zero-error
  rule but lists the 22 real dangling FormIDs as `KNOWN_ERRORS` with evidence and exact
  counts (Starfield.esm PKIN `PRPS.ActorValue` 0x33D x7 and 0x33E x14; SFBGS050.esm
  PNDT `EOVR.Worldspace` 0202B2F4 x1).
## In progress

- **M2.F7 (owner check pending).** `.mcp.json.example` and "Use from Claude Code" in
  `docs/mcp.md` are in; an agent cannot restart Claude Code, so the owner runs:
**M2.F7 result (2026-10-14):** PASS. Owner opened a fresh Claude Code session: `kitcraft` connected with exactly 8 tools (execute, command_line, coverage, diff_plugins, inspect_record, query_records, record_schema, roundtrip), the Default profile preloaded, `record_schema WEAP` returned 50 subrecord definitions (6 typed, 44 opaque), `coverage Constellation.esm` returned 57 records / 8 types. Caveat filed as a lane F request below.

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
- **Lane B round 3** (M2.B4..B8, M2.B10): gate lifted by the owner approval of
  2026-10-14, but **blocked in the 2026-10-14 lane B session**: the session's permission
  policy refused fetching the pinned xEdit files (and two UESP Skyrim pages, KYWD and
  WEAP). No layout was transcribed; see Blockers.

### M3 lane H (evidence: CK observation and oracles)

| Task | State |
| ---- | ----- |
| M3.H1 fixture matrix and session scripts | **done** except F07's recompression candidates (wait for M3.A1's Request): `docs/ck-observations.md` has the ledger format, the matrix F00..F14 with every form resolved (FormID, EditorID, why, the command that found it), the M3.O1 script per fixture, the M3.O2 lists with KitCraft's assumed runtime IDs, and the `fixture.toml` template |
| M3.H2 `cargo xtask observe` | **done**: `cargo xtask observe <fixture-dir>` writes `report.md`/`report.json` in the fixture folder only (SHA-256 of every file; per plugin TES4, `roundtrip --verify`, `coverage`, `validate`; base, `[[compare]]` and `kit_equivalent` comparisons as byte runs, TES4 diff and `kit diff`, our equivalent in both styles with `ck` reported unavailable until M3.E5); `cargo xtask observe new <F-id>` writes the fixture's `fixture.toml`. Tests: synthetic fixture dirs with a fake `kit` (nothing written outside, rerun byte-identical) and the real `kit` when built |
| M3.H4 oracle harness | **Spriggit part done**: `cargo xtask oracle spriggit [--official] [--synthetic] [<plugin>...]` (YAML deleted unread; compared with `kit diff`, `coverage`, `roundtrip`, byte and TES4 diffs); run 2026-10-14 on the 5 smallest official files, 3 hand-built synthetic plugins, 3 `kit set` overrides and the rename plugin; `docs/oracles.md` has the build, fixture set and 11 disagreements (S1: 885 lstring-sized values zeroed in localized files; S3: our TES4 `DATA` after `MAST`, see Requests). M3.O1 fixtures and the overhaul build: rerun when they exist. **SF1Edit part done** (2026-10-14, after M3.O3): `cargo xtask oracle sf1edit [--official] [--synthetic] [<plugin>...] [--types ..] [--sample N]` runs xEdit 4.1.5f's console dumper (`xDump64.exe`; `KITCRAFT_SF1EDIT` = `xSFEdit64.exe` or the dumper) on a scratch Data copy and compares with `kit query`/`inspect`; the editor's script mode is **blocked** headless (Module Selection window; without `-script`, the developer message), documented in `docs/oracles.md`. Results: all records, FormIDs, signatures and EditorIDs agree where xEdit can read the file; 711 decoded members agree on `Starfield.esm` WEAP/AMMO/PROJ/COBJ/OMOD (50 each) and 191 on the four readable small official files (every record), 1 disagreement (X8, ours: lossy cp1252 lstring text); version skew: GPOG/GPOF unknown, SFBGS047 (light master) not loadable, 51 WEAPs with an undecodable component; the coverage-gap list per round 3 type is in `docs/oracles.md` for M3.B11 |
| M3.H3 findings, M3.H5 CK feature ranking | **blocked on M3.O1** (the owner's CK session; `docs/ck-observations.md` has the resolved script, `cargo xtask observe new <id>` the templates) |

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

- **Lane B round 3 needs the pinned xEdit files on disk.** Downloading
  `Core/wbDefinitionsSF1.pas` and `Core/wbDefinitionsCommon.pas` at `9fb01688...` from
  raw.githubusercontent.com (to the agent's target dir, outside version control) was denied
  by the agent session's permission policy on 2026-10-14. The owner either allows that
  fetch for the lane B agent, or places the two files (outside the repo) and names the
  path. Until then M2.B4..B8 and M2.B10 cannot start.

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
| M2.O3 | Install SF1Edit (agents do not download executables)        | **done 2026-10-14 as M3.O3**: xEdit 4.1.5f at `C:\Modding\tools\xEdit\` (older than the pin; `docs/oracles.md`) |
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

- **To lane F (from the F7 check):** `coverage`, `diff` and `roundtrip` reject a bare plugin filename and need the full Data path, while `load`, `inspect` and `query` fall back to KITCRAFT_DATA_DIR. Make bare names resolve the same way in every command (M4.F1 or earlier).

Agents append here when they need a change in a crate they do not own.

- **Lane H -> lane D (strings), from the SF1Edit run (X8).** Some official string entries
  are Windows-1252, not UTF-8 (`Starfield - Localization.ba2` dlstrings `0x000301B6` holds
  byte `0x92` for an apostrophe); our `text` shows U+FFFD there while xEdit shows `’`.
  A Windows-1252 fallback for non-UTF-8 entries (bytes stay in `Resolved.bytes`) would
  make the text agree; until then the oracle counts it as one disagreement of ours.

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
  can now decode payloads instead of classifying by signature (**`coverage` done in
  M3.F2**; `inspect` is M3.D6, lane D). API changes from round 2
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
- ~~**Policy -> lane C.** `coverage` / `cargo xtask parity` should show `confidence`;
  `reference` values should surface in `inspect`/`query` limitations.~~ Coverage and parity
  half done: M2.F9 ("Definitions by confidence") and M3.F2 (`decoded_by_confidence` per
  record type, "Decoded through the definitions"); the `inspect`/`query` half is M3.D3
  (lane D), see the next-but-one entry.
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
  `query` results do not yet. **Done (lane D, M3.D3):** `inspect` and `query` name the
  `reference`-confidence members behind the values they show or test.
- **Policy -> M2.** Install SF1Edit (matching the pin if possible, else record the build) for
  `query` results do not yet (M3.D3).
- **Policy -> M2 (now M3.O3).** Install SF1Edit (matching the pin if possible, else record the build) for
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
- ~~**Lane B -> lane F / coverage owner (FYI, done in lane B's branch).** The engine coverage
  test now counts EDID, FULL and GMST DATA as `typed` (expectations updated in
  `crates/engine/src/handlers/coverage.rs`). For M2.F9: per-member confidence is
  `SubrecordDef::provenance.confidence` (also on groups, fields, records); a union member
  is `FieldType::Union`, not opaque.~~ Used in M2.F9 (`schema`) and M3.F2 (`coverage`).
- ~~**Lane D -> lane F.**~~ Answered in lane F round 3 (below). (1) `inspect`'s `plugin` is no longer required, so it is no longer
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
- **Lane D -> lane H / owner (M3.O2 script, from M3.D6).** BlueprintShips-SFBGS050.esm is
  localized but ships no string tables; its 66 LCTN and 30 GBFM names (FULL) resolve
  nowhere in KitCraft. In game, a location from it (for example the first unresolved LCTN
  `kit inspect` lists) would show whether the game reads its names from SFBGS050's tables
  (30 of the 96 IDs exist there) or shows none. **Lane H:** in the M3.O2 script
  (`docs/ck-observations.md`, step 3) with `BlueprintInteriorSFTER_TerranShip_Generic_TESTLocation`
  (`FULL` 0x0002E9C4; none of the 66 LCTN name IDs is in `sfbgs050_en.strings`).
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
- ~~**Lane E -> lane F (tests touched, catalog closure).**~~ Done in lane F round 3 (see
  the end of this entry). `set`, `validate` and `build`
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
  **Done (lane D, M3.D1):** (1) `Loaded::missing_active_plugins` (the limitation text stays
  for readers, but nothing needs to match it) and `Loaded::source_check(form)`
  (`kitcraft_doc::SourceCheck { winner, blocking, unrelated }`, `is_safe()`): refuse on
  `blocking`, report `unrelated` as limitations (M3.E1). (2) `Resolved::bytes` and
  `Resolved::utf8` (M3.E2).
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
- ~~**Lane G -> lane F (FYI, review round 2).**~~ Answered in lane F round 3 (end of this
  entry). `ba2.extract` gained `max_entry_bytes` and
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
- ~~**Lane G -> lane F.**~~ Answered in lane F round 3 (end of this entry; (1) is moot: no
  catalog command is `Planned`, and those tests build their own spec). (1) Tests that used `ba2.list` as their example of a `Planned`
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
  **Lane F (M3.F2):** answered for `coverage`, which runs the same decode + re-encode check
  per record (with `Record::extra()`, see below) and feeds `cargo xtask parity`.
- ~~**Lane A -> lane B.** Please generate an accessor for the raw subrecords a typed record
  kept: `generated::records::Record::extra(&self)`.~~ Done in M2.B3 (`Record::extra()` is
  generated for every defined type).
- **Lane F -> lane A (M3.F2).** `esm::bridge::extra_of` still matches five variants (GLOB,
  GMST, KYWD, TES4, WEAP), so `check_subrecords` misses `NotDecoded` conflicts in AMDL,
  AMMO, COBJ, OMOD and ZOOM (and in every round 3 type); `typed.extra()` covers them all.
  `coverage` (engine) does not depend on it.
- **Lane E -> lanes D, F (M3 round 1).** (1) Lane F: new catalog command
  `overhaul.scaffold` (alias `overhaul`, session handler registered in
  `handlers::session_defaults`), `build` gained `plan`, `validate` gained
  `plugin_archives` and `.ba2` inputs in `scripts`; `docs/mcp.md` has their sections, the
  command list (M3.F3) may want them. **Lane F:** (1) answered: the sections are the
  command list; no shortcut tool (`execute` and `command_line` reach it, and the tool
  descriptions and `kitcraft://commands` list it from the catalog). (2) Lane D: `build`'s `via` follows typed FormID
  members through `VersionSubject::lookup` locally (`handlers/build.rs` `follow`); M3.D4's
  `follow(form, member path)` should replace it when it lands. M3.E1/E2 wait for M3.D1.
- **Lane E -> lane B (round 2).** `crates/engine/src/edit/lstrings.rs` is a corpus-built
  stand-in for lstring typing the schema lacks (105 signatures, e.g. ACTI/FLOR/FURN/NPC_
  `ATTX`, NPC_ `SHRT`, INFO `NAM1`, MESG `ITXT`, QUST `CNAM`, WEAP `WABB`). As round 3
  transcribes definitions from the pinned xEdit file, please type these as `lstring`
  (`confidence = "corpus"` is already backed by `lstring_table_matches_the_corpus`); the
  schema then wins and the table entry becomes redundant.
- **Lane H -> lane E (and the `docs/corpus.md` "TES4 header" text).** Our patches write
  `TES4` as `HEDR CNAM MAST DATA INCC`, but no official file has a `DATA` after `MAST`: a
  walk of all 14 `TES4` records (13 with masters) finds none, as `crates/schema/defs/TES4.toml`
  says ("MAST is never followed by DATA"), while `docs/corpus.md` says "Every MAST is
  followed by an 8-byte DATA". Spriggit 0.41.0 drops our `DATA` on a round trip
  (`docs/oracles.md` S3). Suggest: correct the `docs/corpus.md` sentence, and either stop
  writing `DATA` or list it as a stated deviation in "Writing rules" until M3.O1's F01/F02
  show what the CK writes (`docs/ck-observations.md`). Changing it changes the rename
  plugin's SHA-256.
- **Lane H -> lane A (M3.A1).** File the F07 recompression candidates (up to 5, CK-openable
  in a dialog) as a Request here; lane H adds them to F07 in `docs/ck-observations.md` and
  the `observe new F07` template.
- **Lane A -> lane H (M3.A1, F07 candidates for M3.H1).** Five compressed records from
  `Starfield.esm` that zlib-rs level 9 does not recompress identically and that the CK
  opens in a dialog, for F07's "one trivial edit each" (all class E in `docs/gaps.md`:
  the last match differs). Each override then shows whether the CK writes the original
  stream, ours (level 9), or a third; compare the CK's body with the original and with
  `deflate(level 9)` of the same decompressed bytes (and say whether the CK recompressed
  the edited record at all, or stored it uncompressed).
  1. NPC_ `0000697A` `UC_NA_CL_TolimanScene_Friend` (3,858 bytes inflated, 1,468 stream;
     the typical NPC_ case: last match 35 at distance 38, ours 37).
  2. NPC_ `002BF312` `Traits_OctopedeA_HornedCrab_Small` (645 / 343; the smallest NPC_
     miss).
  3. NPC_ `00196BB2` `PCM_Porrima_Porrima-III_Predator01` (1,379 / 667; the changed
     distance also changes the Huffman tables, so the streams differ from byte 4).
  4. CELL `0027459B` `LC125SonasHome` (interior cell; 330 / 270; last match length 4).
  5. CELL `000090AF` `SM015DeimosStaryardExterior` (36,764 / 12,614; the largest miss
     outside NAVM).
  The 12 level-8 NAVM and the unexplained NAVM (`001BA29F`) are navmeshes of cell
  `00000025`, not dialog-editable, so F07 does not cover them.
- **Lane A -> lane F (M3.A2 landed; for M3.F1).** `RoundtripOptions::decode:
  Some(DecodeLimits)` (or `RoundtripOptions::with_decode()`) is the driver half of
  `roundtrip --decode`: a compressed body that does not inflate to exactly its declared
  size, whose stream lacks its end marker or checksum or is followed by other bytes, or
  whose subrecords do not parse or re-encode, comes back as `report.mismatch` (so
  `identical()` is false and the CLI's exit 1 applies) with `expected` and `actual` both
  `None`, `offset` where inflating stopped (the body start for size and content faults),
  and `path` ending in ` > compressed body: <fault>` (e.g. `GRUP LAND (type 0) @330 >
  LAND 00000501 @439 > compressed body: invalid zlib stream: ...`). A decode limit
  (`DecompressedTooLarge`, `TooManySubrecords`) is still an `Err`. No public type changed
  (`Mismatch` and `RoundtripReport` have the same fields), so the engine compiles as is;
  for an end-to-end exit-1 fixture, a compressed record whose last stream byte is flipped
  (Adler-32) is the simplest (`driver::tests::decode_reports_a_corrupt_well_sized_stream`).
- ~~**Planet schema lane -> lane D.** `kitcraft_doc::typed::field_type` looks a layout up by
  signature, so it cannot tell a top-level member from the same signature at another
  position: PNDT has CNAM, FNAM and GNAM twice with different layouts (it returns the
  "ambiguous" error), and the component blocks of PCBN, PKIN and GBFM contain NAM1..NAM5,
  VNAM, CNAM and ANAM that mean something else at the top level (it decodes them with the
  top-level layout). `typed_reader_agrees_with_generated_and_lstrings_resolve_all_files`
  (`#[ignore]`) now panics at `Starfield.esm PKIN:0000B065 VNAM` (a 1-byte VNAM inside the
  placement-properties component). Please resolve positions with
  `generated::records::match_members` (the decoder's own placement) and skip `any_until`
  bodies. Until then the GBFM object-template FULL is left out of the schema (it made the
  small-file test fail on SFBGS004 GBFM FE000046).~~ Done in M4.BX1 (positional layouts in
  `kitcraft_doc::typed`, see Done); the GBFM object-template FULL can be typed again
  (family lane BP).
- ~~**Planet schema lane -> lane E.** (1) `edit::fields::layout_for` has the same
  signature-only limit (PNDT CNAM/FNAM/GNAM return the "different layouts" error, so
  `inline_lstrings` refuses to copy a PNDT record from a localized source). (2)
  `validate_corpus_official_files` (`#[ignore]`) now reports errors on official files,
  because more FormIDs are typed: 21 `formid.dangling` in Starfield.esm (PKIN `PRPS`
  actor values 0x33D/0x33E, which no file defines; ShatteredSpace has 2 such warnings) and
  1 in SFBGS050 (PNDT `EOVR` entry naming WRLD 0202B2F4, which SFBGS050 lacks). They are
  real corpus facts; decide whether object IDs below 0x800 are engine-reserved (a grade D
  hypothesis worth a CK check) and how the acceptance test treats official danglings.~~ Done in M4.BX1: (1) `edit::fields::layout_at`/`layouts` resolve by
  position; (2) the acceptance test lists the 22 as `KNOWN_ERRORS` with evidence and exact
  counts. Still open as a CK question: whether object IDs below 0x800 are engine-reserved.
- **Planet schema lane -> lane A.** Same as lane F's entry above: `bridge::extra_of` lists
  five types, so `check_subrecords` reports no `NotDecoded` for the 15 planet types either.
- **Planet schema lane -> lane H (CK observation).** Questions only the CK can answer
  (details in `docs/schema/planet-family.md`, "Open questions"): (1) which CK fields map to
  the placement-properties component (ZNAM is unnamed upstream) and how the CK writes it on
  a new PCCN/PCBN/PKIN/LVLP; (2) whether the CK ever writes PCCB child lists or only
  PRTN/PRVN when nodes are added or reordered; (3) how the CK shows the 4 PCCN without PCCC
  and the 5 whose PCCC names a REFR; (4) which of PKIN's two OPDS slots the CK edits;
  (5) which CK checkboxes set the record flags named only upstream (PKIN, GBFM, STDT,
  LVLP); (6) the unnamed values (BIOM BNAM/GNAM slots, PNDT PPBD/HNAM slots, TRAV DNAM
  `Value4`, SFBK NAM1, PKIN FNAM bit 0); (7) RSPJ `NumberCreated`/`Tier`, 0 everywhere.
