# Status

Updated: 2026-10-15 (reconciled against `main` at `01c77d8`)

## Owner next actions

In-game checks use only the MO2 profile **KitKraft Dev**, each on a disposable save. Never
use `Default` (AGENTS.md, "Owner's live game is off limits").

| # | Action | Unblocks | State |
| - | ------ | -------- | ----- |
| 1 | In-game check of the rename plugin `OldSovietMarksmanRifle.esm` (M2.O6) | M2 exit | **done 2026-10-14: PASS** |
| 2 | In-game check of the first POI (M4.O2). Install `out\poi\package` through MO2 and check the hashes against its manifest. Then in the console: `coc CityNewAtlantisApartmentTEMPLATE`, then `player.moveto KitKraftFirstPOICrate` (or `player.moveto 28001000`) and `prid` it. Take a screenshot and run the persistence steps in `docs/poi/first-poi.md`. In the same session, install the rename package (W12, `docs/packaging.md`) | M4 exit (first POI, W2, W5, W12) | pending (owner) |
| 3 | In-game check of overhaul build 2 (M3.O5, steps and checklist in `docs/overhaul/build-2.md`, "In game"; package `out\overhaul\package\KitKraftGunOverhaul-0.2.0.zip`), on `KitKraft Dev` with a disposable save | M3 exit (overhaul) | pending (owner); build 2 landed 2026-10-15 |
| 4 | ~~Answer the brief rows B8, B11 and B12~~ | overhaul build 2 (M3.E9 rerun) | **answered 2026-10-15**: B8 dropped, B11 base 18 (drum untouched, 48), B12 as built; applied in build 2 |
| 5 | Close every Claude Code session that has the repo open. Then run `C:\Modding\scripts\finish_kitkraft_move.ps1`. It renames `C:\Modding\kitcraft` to `C:\Modding\kitkraft`, re-points the MCP registrations, and runs `git worktree prune`, so run it only when no lane worktree is in use. Open new sessions on `C:\Modding\kitkraft` | the folder name every doc already uses | pending (owner) |
| 6 | Decide when to start the workbench (lane W, M4.W0..W6, ADR 0007 Amendment 1). The M4.W0 gate needs M4.D1 (`refs`) and M4.D2 (placement context), and neither is on `main`. The deferred CK sessions (M3.O1, M4.O1) wait for M4.W2..W5 | M3.O1 and M4.O1 (CK), with them every `ck-diff` upgrade | **decided 2026-10-15: started**. Lane W built the read-only workbench (`kit ui`, `docs/workbench.md`) |
| 7 | Rule on two licenses in the workbench's dependency tree (ADR 0007, "Dependency list"). First, **BSL-1.0** (`clipboard-win`, `error-code`): eframe always pulls these in for the copy buttons; the license is permissive but is not named in the MIT/Apache/Zlib/BSD rule. Second, whether the optional `default_fonts` feature may ever be enabled (OFL-1.1 and Ubuntu Font Licence). It is off: the window uses the Windows system fonts | the first workbench merge to `main` | open (owner decision) |
| 8 | Run the workbench walkthrough against `KitKraft Dev` when the workbench changes (it is `#[ignore]`d, so `cargo xtask ci` skips it). PowerShell: `$env:KITKRAFT_WORKBENCH_PROFILE = 'C:\Modding\MO2-Starfield\profiles\KitKraft Dev'`, then `cargo test --release -p kitkraft-ui-egui --test dev_profile -- --ignored --nocapture`; it reads the profile and writes nothing but the engine's index cache (`docs/workbench.md`, "Panels") | confidence that `docs/workbench.md` matches the real profile | pending (owner), after each workbench round |

**Director ruling pending owner confirmation (2026-10-15):** eframe's clipboard support brings `clipboard-win` and `error-code` under BSL-1.0 (Boost). BSL-1.0 is permissive with no copyleft and is compatible with MIT/Apache-2.0; applying owner clarification 2 (no pause unless the license category differs), the workbench was merged with this noted. Owner: confirm, or ask for the clipboard feature to be dropped.

## Current milestone: M3 closing, M4 in progress

M0, M1 and M2 are done. M2 closed on 2026-10-14: M2.F7 and M2.O6 both passed. M3 cannot
close yet. Its overhaul check waits on the owner (M3.O5, row 3 above). Its CK
criteria are carried until the CK session, which the owner deferred until the workbench
exists. M4 runs on the critical path for the first POI. "M3 and M4 exit criteria" below
checks every "done when" bullet of `plan/execution-plan.md` against `main` at `01c77d8`.

## M3 and M4 exit criteria (reconciled 2026-10-15 at `01c77d8`)

States: **done** (on `main`), **pending-owner** (needs an owner action above),
**carried** (open, with the task that carries it and the reason). Not rerun for this
reconciliation. Each "done" cites the evidence that lane recorded.

### M3 is done when

| Criterion (`plan/execution-plan.md`) | State | Tasks and evidence |
| ------------------------------------ | ----- | ------------------ |
| Round 3 types (WEAP AMMO PROJ AMDL OMOD COBJ ZOOM LVLI FLST INNR) typed, sourced, with a confidence; 0 conflicts on all 14 files; 105 lstring signatures typed | **done** | M3.B1..B5, B7, B8; merged in M4.BX1. Parity: failed 0, 0 typed conflicts. Round 4 typed the OMOD property values (sibling-field unions) |
| KYWD and GMST remainders typed | **done** | M3.B6, lane B round 5: KYWD from the pinned xEdit file (L13297-L13312): CNAM `ColorRgba`, DNAM/ENAM zstrings, TNAM `u32` (as LCRT), DATA FormID (AORU), VMAD at `reference`, FNAM opaque (upstream gives no layout); GMST XALG `u64` (L12357) and the DATA decider cited (Common L5887-L5911). Parity: KYWD 22,948 and GMST 4,858 decoded occurrences, 0 failed, 0 re-encode differences; `docs/gaps.md` row closed |
| Staged xEdit files deleted and recorded | **carried** | To the end of M4. The schema family lanes still transcribe from `C:\Modding\tools\xedit-ref\` (lane B round 5 too) |
| `docs/ck-observations.md` entries for every M3.O1 fixture; M3.H3 targets answered; TES4 `SNAM`/`BNAM`/`CHGL` settled; CK fixtures roundtrip (M3.A4) | **carried** | M3.O1, H3, A4. The owner deferred the CK session until the workbench exists (M4.W2..W5). Lane H's side is ready (M3.H1, M3.H2) |
| The 160 recompression misses settled or restated with evidence | **done** (restated) | M3.A1: classified E/N/U with the study in `docs/gaps.md`. F07 still checks it in the CK (carried with M3.O1) |
| `style: "ck"` in `set`/`build`, matching the CK's bytes or explained (M3.E5) | **carried** | M3.A3, M3.E5, with M3.O1 |
| `docs/ck-parity.md` ranks the CK features (M3.H5) | **carried** | M3.H5, with M3.O1 |
| Spec v2 and `kit overhaul` `Implemented` | **done** | M3.E6, M3.E7 |
| First overhaul build from the owner's brief validated (M3.E9) | **done** (build 2) | Build 1: 8 implemented, 4 partial, 2 open. Build 2 (2026-10-15, `docs/overhaul/build-2.md`): the OMOD values lane B round 4 typed and the owner's answers; 13 rows implemented, B8 dropped; validate 0/0, roundtrip identical, packaged 0.2.0 |
| Owner's in-game overhaul result recorded (M3.O5) | **pending-owner** | Row 3 above (build 2) |
| `docs/oracles.md`: the Spriggit run, and the SF1Edit run with M3.B11's tables | **done** | M3.H4 (both harnesses), rerun after the family merges (M4.BX1). M3.B11 reruns as members get typed. A build at the pin is the owner's (`docs/gaps.md`) |
| Every hardening task done or carried; `docs/gaps.md` says what M3 closed | **carried** | Done: M3.A1, A2, D1, D2, D3, D6, E1, E2, E3, F1..F4, G1..G3. Carried, each with its `docs/gaps.md` row: M3.A5 (fuzzing), M3.B9 (other deciders; round 4 did the sibling field), M3.B10 (`count_from`), M3.D4 (`follow`, `refs`: done as M4.D1), M3.D5 (M3.O1, M3.O2), M3.E4, M3.E10 (patch remapping) |
| ADR 0007 committed; M3.O6 recorded | **done** | M3.F4; M3.O6 decided 2026-10-14 (Amendment 1). M3.F5 is not built, by that decision |
| `cargo xtask ci` green; corpus roundtrip identical; parity regenerated; STATUS says M4 | **done** at `01c77d8` | Gates as recorded at the lane B round 4 merge (`39ca2a2` regenerated parity). STATUS: this section |

### M4 is done when

| Criterion | State | Tasks and evidence |
| --------- | ----- | ------------------ |
| QUEST, ACTOR, ITEMS/WORKSHOP typed, with WORLD, PLANET and round 3 merged; M4.BX1 after every merge with 0 typed conflicts | **done** | Six families: round 3, BW, BP, BQ1, BC1, BI1, each merged by M4.BX1. `unordered` record mode for CELL/REFR/ACHR (parity failed 1,712,200 -> 0). Lane B round 4 added the OMOD/OBTS property unions (8,669 entries) |
| `docs/record-parity.md` shows the per-family counts | **done** | Lane B round 5: `cargo xtask parity` opens the document with a "Families" table (weapons, world, planet, quest, actor, items, container/other: record types, definitions, records, typed/opaque/unknown occurrences, total); the family lists live in `xtask/src/parity.rs` (`FAMILIES`, a test checks each type has a definition and sits in one family) |
| M4.BX2 inventories (`LAYR RFGP WKMF IRES MESG`) | **carried** | M4.BX2, lane B. Not on `main` |
| Builder: computed cell/world paths, rebuild gate (M4.A2); allocator and reference composition tested | **done** | M4.A1, A2, A3, A5, with the Fable review fix round |
| NAVM passthrough on a synthetic plugin, and on F22 if it exists | **done** (synthetic) | M4.A4. The F22 half is carried with M4.O1 (deferred) |
| `new`, `clone`, `place`, `package`, `testplan`, `papyrus.events` `Implemented` and on MCP | **done** | M4.E1, E2, E6, G2, and `package` (lane G). `deploy --dev` is live and reviewed too (M4.E11, ADR 0009 Amendment 1) |
| `planet` `Implemented` | **done** | M4.D3, lane D (`kit planet`, alias `pcm`; Done, "Lane D, M4"). Unblocks M4.E10 |
| Spec v3 builds deterministically | **done** | M4.E3 (two builds identical), Fable review E fixes |
| Cell rules run with 0 errors on the official files | **done** | M4.E4. 0 unexplained errors; one known error (`ref.orphan` on 0x1FB) |
| First POI built, validated and packaged | **done** | `out\poi\KitKraftFirstPOI.esm` (`docs/poi/first-poi.md`), reviewed |
| First POI visited in game, persistence steps recorded in `docs/game-observations.md` | **pending-owner**, **carried** | M4.O2 (row 2 above). The ledger file is M4.H4 (lane H), not on `main` yet. Until it lands, record the result in STATUS |
| P0 first deliverables W1..W6, W12 | partly | W1 **done** (M4.E1, `KitKraftW1.esm`). W2 and W12 **pending-owner** (M4.O2). W3 **carried** (M4.E7 not started). W4 recipe built (M4.E8), **pending-owner** (M4.O4). W5 **carried** (M4.H3 `docs/design/persistence.md` not on `main`; then M4.O2). W6 **carried** (M4.G2 done; M4.E9 waits for a typed script-attachment block, VMAD opaque) |
| P1 deliverables W7..W10 done or carried; W11 scoping document | partly | W7 **done** (M4.D3: `kit planet`, `docs/design/planet-content.md`). W8 **carried** (M4.E7). W9 recipe built (M4.E8), **pending-owner** (M4.O4). W10 `refs` **done** (M4.D1). W11 **done** (M4.G3, `docs/research/asset-formats.md`) |
| M4.O4 in-game results recorded | **pending-owner** | After E7/E9 land; the M4.E8 recipes are ready |
| `docs/design/persistence.md`, `docs/design/planet-content.md`; `docs/research/asset-formats.md` | partly | asset formats **done** (M4.G3). Persistence **carried** (M4.H3), planet content **done** (M4.D3) |
| M4.O3 and M4.O5 recorded | **done** | ADR 0008, ADR 0009 (2026-10-14) |
| Lane W | **done** (read-only part) | Started 2026-10-15 (row 6). `crates/ui-egui` + `apps/kit` (`kit-ui`, `kit ui`) have the Load, Records, Diff, Diagnostics, Validate and Schema panels (`docs/workbench.md`). Carried: M4.W0 shape tests (lane F), `build --plan` (M4.W6), `refs`/placement context (M4.D1 done in the engine; panels: Request to lane W; M4.D2). Licence ruling: row 7 |
| CK fixtures F20..F28 (M4.O1, M4.H2) | **carried** | Deferred by the owner with M3.O1 until the workbench exists. The first POI does not need them |
| `cargo xtask ci` green, corpus roundtrip identical, parity regenerated, STATUS says M5 | **carried** | To M4 close |

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
- M1.8 (lane B) definition format: TOML loader + validator in `kitkraft-schema`, format in
  `crates/schema/defs/README.md`; `source`/`evidence` must use the ADR 0005 forms.
- M1.9 (lane B, rescoped) seed definitions: TES4 and the GRUP header typed from
  `uesp-skyrim` pages and confirmed on the corpus; GMST, GLOB, KYWD, WEAP as corpus
  inventories (order, cardinality, BFCB..BFCE and OBTE..STOP blocks), every payload opaque;
  WEAP layout pending ADR 0005 (ck-diff). Corpus findings: TES4 MAST has no DATA in any
  official ESM; GRUP type 10 occurs (unnamed, preserved).
- M1.10 (lane B) codegen: `kitkraft_schema::generate(&Schema) -> String`; golden tests for the
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
  the catalog; resources `kitkraft://commands`, `kitkraft://load-order`; argument and
  command errors as `isError` results, engine bugs `-32603` (`docs/mcp.md`); `kit mcp
  [--profile DIR]`; in-process pipe client test and a spawned-binary test (with
  `KITKRAFT_DATA_DIR`: `inspect_record` on `Constellation.esm` passes, `query_records`
  answers `not_implemented` until M2.D11); `Status::Deferred { reason, milestone }` and
  the `catalog_closure` test (every `Planned` command must be listed in `PLANNED_IN_M2`
  with its owning M2 task; nothing is deferred yet because every command has an M2 owner).
  Review round 2 (Fable, no blockers) fixed: `roundtrip --out` opens with `create_new`
  unless `overwrite: true`, and `destructiveHint` is `true` on the tools that can write;
  `coverage` gained `signatures` and `top`, and MCP defaults them to `false`/`20`
  (`backend::MCP_DEFAULTS`; `Starfield.esm`: 18 KB instead of 695 KB, text leads with
  `data.summary`); `execute` reports `unknown_command` before other violations and takes
  `args: null` as `{}`; a `not_implemented` `--profile` preload stays out of the
  instructions; `docs/mcp.md` states that paths are not confined to `KITKRAFT_DATA_DIR`.
  **Round 3 (2026-10-14):** M2.F8 closed: every catalog command is `Implemented`,
  `PLANNED_IN_M2` is gone and `catalog_closure` fails on any `Planned` command; partial
  commands say so in `limitations` (inspect single-plugin mode, diff/conflicts compare
  stored bytes). `data_dir` defaults to `KITKRAFT_DATA_DIR` on every command that takes it
  (a profile prefers MO2's `gamePath`); `validate` searches a standalone plugin's masters in
  the load order, next to the plugin, then the Data dir, and resolves a bare plugin name
  there (`kit validate --plugin out\X.esm` now passes with only the variable set). M2.F9:
  `schema` command (`kit schema --type WEAP [--member KWDA] [--deep]`, alias `defs`, MCP
  tool `record_schema`): members with type, source citation and confidence, top level by
  default; `cargo xtask parity` adds a per-confidence table from it. Resources:
  `kitkraft://validate-rules` (rules, severities, denylist, docs); `kitkraft://commands`
  gains `cli` and `tools`. The flaky `kit_mcp_on_an_official_file`: spawned servers use a
  private `KITKRAFT_CACHE_DIR` under the test target dir (no more sharing the user's index
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
    errors, late masters reported); D5 index cache (`%LOCALAPPDATA%\kitkraft\cache`, path +
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
    `diff_plugins`; `load` fills `kitkraft://load-order`).
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
  - E10 rename: `C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm` (2,197 bytes; `TES4`
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
- M4 lane A (container), 2026-10-14:
  - M4.A1 cell and world group structure (`docs/corpus.md` "Cell and world groups";
    `kitkraft_esm::cells`; `crates/esm/tests/cells.rs`, every rule asserted on every group
    and record of the 14 files, 27 s release): group types 1..10 with parents, labels and
    contents; type 10 = quest children (`DLBR`, `DIAL` + type 7, `SCEN`); persistence =
    flag 0x400 and type 8 together (115,985 / 6,714,413, no exception); a world's
    persistent references in its persistent cell (the flag-0x400 `CELL` first in type 1);
    interior block/sub-block = last two decimal digits of the object ID (17,020 cells; `FE`
    object IDs are 12 bits, decided by `SFBGS047` `FE001016`); exterior sub-block = grid/8,
    block = sub-block/4, labels y low / x high (22,682 cells); grid = `XCLC` i32 pair, x
    first, and cell width 100 units (99.983 % of 2,038,073 exterior references), both found
    by corpus search, not from a definition; order rules; the official-pipeline vs
    `BlueprintShips` differences (`PlacementStyle`). Not waiting on BW: the two layout
    facts used (`XCLC` grid, `DATA` floats 0-1) are corpus-derived and named as such.
  - M4.A2 computed placement paths and the rebuild gate: `cells::placement(&RecordHeader,
    &Parent, &PlacementStyle)` (cell children persistent by flag 0x400; refusals where the
    corpus never shows a combination), `cells::cell_place_in` / `persistent_cell` from an
    index; `Plugin::upsert_styled` and the streaming upsert (`RewriteOptions::placement`)
    create blocks, sub-blocks, world, cell, quest and topic children and records in corpus
    order, with style stamps and the placeholder top-level groups. **Rebuild gate**
    (`crates/esm/tests/rebuild.rs`): every top-level group of all 14 files rebuilt from
    an empty plugin through computed paths, in file order and shuffled, is byte-identical
    (12 pipeline files including the whole of `Starfield.esm`, not a sample; the two
    `BlueprintShips` files after 69 listed differences: 17 empty cell children groups, 52
    off-majority group stamps). 116 s release; the five small files whole in every gate.
    `Plugin::upsert` keeps its signature (official style); INCC stays the caller's (lane E
    already adds inserted interior cells).
  - M4.A3 new-form allocator `kitkraft_esm::forms::NewForms` (`for_index`, `for_plugin`,
    `allocate`, `peek`, `write_hedr`): own FormIDs `<master count><object ID>` above both
    the `HEDR` next object ID and the highest own object ID, `HEDR` advanced keeping its
    high byte; small/medium plugins, both scale bits, 253+ masters and object IDs past
    `0xFFFFFF` are `Error::Refused`. Corpus defaults named in `forms::LIMITATIONS` until F06
    (and F11/F26). `tests/forms.rs`: every small/medium official file refused; the 6 full
    ones start one past their largest own object ID (Starfield.esm `003FFD5A`).
    `Tes4::set_next_object_id` added.
  - M4.A4 NAVM passthrough `kitkraft_esm::navmesh`: `import_navmeshes(&source, &mut
    target, &style)` (in memory) and `navmesh_placements(&source, &target_index,
    read_cell, &style) -> Vec<Placed>` (for the streaming upsert) copy every `NAVM` of a
    CK-saved plugin unchanged (compressed bodies as written) into the temporary children
    of the same cell in the target, `NAVM` first; refused (`Error::Refused`) when the
    master lists differ in any way (payload FormIDs are opaque), when the target lacks the
    cell, or for a `NAVM` outside a cell's temporary children. Synthetic test: a "CK"
    plugin minus its navmesh plus the import equals the CK plugin byte for byte, in memory
    and streamed. `tests/navmesh.rs` runs on `observe\<date>-F22\` and skips with one line
    without it (F22 not observed yet). `NAVI` is not imported (F22 will show whether the
    CK changes it).
  - M4.A5 reference composition `kitkraft_esm::refs`: `compose_reference(&RefSpec,
    allow_reference) -> ComposedRef { record, members, limitations }` and
    `ComposedRef::placed(CellPlace, cell, &style) -> Placed` (persistent -> the cell's
    type 8, which for a world is its persistent cell). Raw subrecords in corpus order
    (`EDID`, leading, `NAME`, extra, `DATA`, trailing); flags 0x400/0x800 from the spec.
    Each member's confidence comes from the schema's `REFR`/`ACHR` definition when one is
    built in (lane BW's, after BX), else from the corpus: `NAME`, `DATA` X/Y, persistence
    `corpus`; `DATA` Z, rotation, flag 0x800 and raw flags below corpus, refused unless in
    `allow_reference`. So until BW+BX land, a POI placement must allow
    `DATA.Position.Z` and `DATA.Rotation`; afterwards nothing changes in code.
    `tests/refs.rs`: reference shape on all 6,734,456 references; 9,144 sampled
    references composed from their decoded values are byte-identical.
  - M4.A6 (corpus half; F23 still open for the CK's own choices): `ONAM` membership rule
    found and asserted on every record of the 14 files (`tests/onam.rs`): listed exactly
    when the record overrides a record its master has and sits in group type 7, 9 or 10
    (`cells::ONAM_GROUP_TYPES`); never type 8, cells or top-level records. The official
    list order is not reproducible (neither file nor FormID order). Writer:
    `cells::onam_forms(&plugin, overrides_master_record) -> Vec<u32>` (file order, a
    stated choice until F23) and `Tes4::set_overridden_forms(&mut tes4, &forms)` (after
    the master list, as in every file). Request to lane E below.
  - Fix round after the Fable review of the placement core (2026-10-14): (1) blocker:
    the own FormID prefix counts only full masters (`NewForms` takes the masters'
    scales, refuses unknown ones); (2) a children group without its record is refused,
    `cells::carry_parents` carries the `WRLD`/`CELL` overrides, `upsert_before_owner` is
    the rebuild-only opt-in; (3) existing interior cells keep their observed blocks;
    (4) a persistence toggle moves the record (`Upsert::Moved`); (5) off-cell positions
    are named, bad reference specs refused; (6) `onam_forms_in`, `NavmeshImport`
    master overrides; (7) `PlacementStyle::detect`, flag-0x400 path checks, one
    persistent cell per world, iterative walks, `DecodeLimits` through the navmesh
    import, the blueprint `HEDR` count limitation. Rebuild gate and full roundtrip
    unchanged (see the lane's report). Lane E: new signatures in Requests below.
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
    (12.3 MB) are extracted once into the KitKraft cache, keyed by name, size and CRC-32,
    hostile names refused before any write, and imported after `imports` (zip crate 9,
    deflate only). PapyrusCompiler 4.7.0.5 with only the zip: DefaultAliasOnActivate,
    DefaultCounterQuestA and the noc patch's nocOutfitChangerQuest,
    nocOutfitChangerContainer, SQ_Airlock_QuestScript, CS7_SpaceshipLandingQuestScript all
    compile (first call 14.5 s with the extraction, then 1-2 s each). Gaps row closed.
  - M3.G2 tolerant string-table parse: `kitkraft_strings::ParseMode::{Strict, Tolerant}`,
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
- Lane F, M4 (2026-10-14):
  - M4.F1 bare input names (the F7 Request): `handlers/resolve.rs` resolves an input
    plugin, archive or table given as a bare file name the same way in every command:
    the session's load order (by plugin name), then the working directory, then the Data
    dir (`data_dir`, else `KITKRAFT_DATA_DIR`); a path with a directory is used as given.
    Used by `roundtrip`, `coverage`, `diff`, `ba2.list`/`extract`, `strings.dump`/`merge`,
    `papyrus.disasm` (now session handlers through `resolve::Resolving`, with a
    `data_dir` argument each), `validate` (`plugin`, `scripts`), `inspect`'s single-plugin
    `plugin`, and the `plugins` list of every load-order command (`load()`: `set`,
    `build`, `conflicts`, `query`, ...; a bare name in the working directory now wins over
    the Data dir there, as `validate` already did). A bare name found nowhere is the new
    `EngineError::NotFound` (kind `not_found`, `error.searched` in `kit --json` and MCP).
    Outputs (`out`, `out_dir`, `into`) never resolve, so the write guards are unchanged.
    Tests through `execute` per command (`handlers/resolve_tests.rs`): `Constellation.esm`
    (or its archives, `Starfield - Misc.ba2` for `papyrus.disasm`) resolves from
    `KITKRAFT_DATA_DIR`, and an empty `data_dir` gives `not_found` naming it.
  - M3.F1 `roundtrip --decode` (lane A's M3.A2 Request): `decode` in the catalog, CLI
    (`--decode`) and MCP passes `RoundtripOptions::with_decode()`; with it `data.passed`
    is `identical` (a check, like `verify`), `data.decode` is reported, the first
    limitation states the strict decode, and a decode fault's summary names its path.
    End to end: `kit roundtrip <corrupt> --decode --json` exits 1 with `data.passed:
    false` on a synthetic plugin whose compressed GMST has its Adler-32 flipped
    (`apps/kit-cli/tests/exit_codes.rs`, `roundtrip_decode_exit_1`); both `docs/gaps.md`
    roundtrip rows closed.
- Lane E, M3 (engine `src/edit/`, `src/validate/`, `src/build/`,
  `handlers/{set,validate,build,overhaul}.rs`, `docs/overhaul/`):
  - M3.E3 `validate` hardening: `FE`/`FD` FormIDs resolve through lane D's class encoding
    (`kitkraft_doc::RawRef`; a master of unknown scale before the index and
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
    `missing_active_plugins` in `load`'s listing) and `kitkraft_doc::source`:
    `LoadOrder::source_check(form, missing_active)` / `Loaded::source_check(form)` returns
    the winner with `blocking` and `unrelated` problems (`is_safe()`); `inspect` shows it as
    `winner_source`. Blocking: no or deleted winner, a version's plugin loading before its
    master, a plugin after the winner that failed to load or has unresolved records that
    could be this form (it names the defining plugin and a stored object ID matches). Default
    profile: the rifle `Starfield.esm:0021BBCD` is safe (unrelated: 4 unresolved records of
    `HG_ShipCargoHoldAtHome.esm`, none can be it). `strings::Resolved` has `bytes` (the
    table's exact bytes) and `utf8`; every non-UTF-8 `starfield_en` entry resolves with its
    bytes (`crates/doc/tests/strings_corpus.rs`).
  - M3.D2 (generic part; proven on today's typed members): `kitkraft_doc::fields` compares
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
  - M3.D3: `kitkraft_doc::confidence::reference_members` finds the `reference`-confidence
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
  (`KITKRAFT_UPDATE_GOLDEN=1`) and the builtin-set lists in `crates/schema/tests/defs.rs`
  (`builtin_set_loads_and_matches_the_directory`).~~ Done in M4.BX1 (PLANET merge,
  2026-10-14): all four regenerated on main (`codegen --check` clean); `cargo xtask ci`
  passed with `KITKRAFT_DATA_DIR`; ignored release corpus tests for esm, doc and engine
  pass (M2.A3 typed check 0 conflicts; doc `typed_corpus` 46,713 typed records, 181
  pinned unresolved lstrings as before); `docs/record-parity.md` regenerated: 25 record
  definitions, 50,419,561 subrecord occurrences, 379,633 typed (0 failed), 218,127
  opaque, 49,821,801 unknown (Starfield.esm parity pass 43.6 s).
- Quest schema family (parallel schema lane M4.BQ1, `crates/schema/defs`, 2026-10-14):
  FACT, QUST, PACK, DIAL, INFO, SCEN, SMBN, SMQN, SMEN, CHAL transcribed from the pinned
  xEdit files (one commit per type, every member cited with its line range); new shared
  `common/` definitions ScriptAttachment (VMAD header typed, scripts and fragments raw; a
  type), PackageLocation, QuestReferenceAlias, SoundReference; PLANET's Condition reused.
  Corpus check (14 files, out-of-tree checker over the regenerated decoder): all 267,549
  records of the ten types decode with every subrecord placed, nothing in `extra`, and
  re-encode byte-identically (FACT 953, QUST 2,610, PACK 4,578, DIAL 87,031, INFO
  162,028, SCEN 9,673, SMBN 173, SMQN 471, SMEN 22, CHAL 10; none compressed; about 6 s in
  release for all ten). INFO and DIAL are not order-unstable: every official record
  follows the upstream member order. DIAL and SCEN live only in quest children groups
  (type 10), INFO in topic children groups (type 7). QUST stages, objectives and the
  three alias kinds are typed at `corpus` (W10 acceptance input for M4.D1); aliases
  interleave, so QUST stores runs (`Aliases`: reference, then location, then collection
  aliases per entry; lossless). Opaque by design: VMAD after its header, CTDA unions,
  component blocks (all of CHAL), SCEN action type-specific subrecords (union by action
  type, `any_until` the end marker), PACK input values/targets, INFO ENAM flag sets, QUST
  ALFG/ALAM/create-matching ALCA, unnamed slots (gaps rows). Mod-author guide:
  `docs/schema/quest-family.md`. `cargo xtask ci` passed locally with
  `KITKRAFT_DATA_DIR` and the derived files regenerated (the ignored official-files
  validate test needs BX's known-error update, see Requests). Derived files are left to the
  integrator (M4 family lane rule, BX): `crates/esm/src/generated/`, the
  `BUILTIN_DEFINITIONS` region of `crates/schema/src/lib.rs` (14 new entries: 10 records,
  4 common), `crates/schema/tests/golden/seed.rs` (`KITKRAFT_UPDATE_GOLDEN=1`) and the
  builtin-set lists in `crates/schema/tests/defs.rs`.
- M4.BX1 QUEST merge (integrator, BX3, 2026-10-14): the quest family's full CHAL, DIAL,
  FACT, INFO and QUST win over round 3's lstring-only stubs; every lstring the corpus
  table lists for them is a member except CHAL FULL/DESC, which official challenges carry
  only inside their one base form component (added to `COMPONENT_LSTRINGS`). Shared
  blocks folded: the family's `common/SoundReference` had the same 40-byte layout as round
  3's (one file, both citations). `common/Condition`: CIS1/CIS2 raised to `corpus` (17 and
  210 occurrences in QUST, PACK, SCEN, INFO) and the family's 102,280 CTDA added to the
  evidence. Derived files regenerated (generated code, `BUILTIN_DEFINITIONS`, golden,
  `tests/defs.rs` lists: 80 record definitions, 30 shared). `validate_corpus_official_files`:
  the 22 new Starfield.esm errors are `KNOWN_ERRORS` (SMEN `PNAM` = 0x5B x20, a reserved
  story manager root; `QuestReferenceAlias.ALFR` = 0x14 x2, the player reference class).
  Gates: `cargo xtask ci` with `KITKRAFT_DATA_DIR`; ignored release tests for esm, doc and
  engine all pass (17 min: 14/14 identical, typed check 0 conflicts, `world_family_full`,
  `corpus_rebuild_full`, doc `typed_corpus` 7,254,548 typed records and the 181 pinned
  unresolved lstrings). Parity: 80 record definitions,
  44,743,862 typed (1,712,200 failed, all the
  unordered CELL/REFR/ACHR placements, unchanged), 2,249,507 opaque, 3,426,192 unknown.
  `papyrus.compile`'s source extraction now retries its folder rename briefly (a fresh
  private cache under `.targets\` failed with "Access is denied" while a scanner held the
  new files).
- M4.BX1 position-dependent layouts (integrator, 2026-10-14): `kitkraft_doc::typed` now
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
- M4.BX1 lane B round 3 merge (integrator, 2026-10-14): round 3's ten typed types and 49
  lstring-only stubs on top of the planet family. The planet family's full BIOM, GBFM,
  PKIN, PNDT, RSPJ and STDT win over round 3's stubs; every lstring round 3 listed for them
  is a member except the three official records carry only inside a base form component
  (GBFM HULL, PNDT FULL, STDT FULL: raw with the component data, typed by the engine's
  corpus-table fallback; `COMPONENT_LSTRINGS` in `crates/schema/tests/defs.rs`), and GBFM
  gets back its combination FULL (`reference`, xEdit L6961-L6974) now that layouts resolve
  by position. `common/Condition.toml`: one definition (the planet family's typed CTDA
  struct, round 3's COBJ evidence added), used by COBJ, FLST, LVLI, LVLP, PCBN, PCMT, RSPJ.
  `edit::fields` keeps the positional lookup with round 3's no-fallback expectation.
  `validate_corpus_official_files`: 23 new FLST `LNAM` errors in Starfield.esm, all object
  IDs 0x2D8..0x359 in five actor-value form lists (ENV_imGui_* and
  SpaceshipPartSummedValues), the PKIN PRPS class (engine-reserved actor values, grade D),
  listed as `KNOWN_ERRORS` with exact counts. Gates: `cargo xtask ci` with
  `KITKRAFT_DATA_DIR`, the ignored release corpus tests for esm, doc and engine (14/14
  identical, M2.A3 typed check 0 conflicts, doc `typed_corpus` 7,127,228 typed records and
  181 pinned unresolved lstrings); parity: 72 record definitions, 1,028,119 typed (0
  failed), 26,866 opaque, 49,364,576 unknown subrecord occurrences.
- **World schema family (parallel schema lane, 2026-10-14).** WRLD, CELL, REFR, ACHR,
  NAVM, LCTN and LCRT typed from the pinned xEdit files (`crates/schema/defs/`, nine new
  shared definitions in `defs/common/`), each member cited with its lines and `confidence`
  (`corpus` where the corpus has it, else `reference`). `crates/esm/tests/world_family.rs`
  checks every record of the seven types in the 14 files (6,880,278 records; REFR sampled
  1 in 20 in `cargo test`, all in `--ignored` release, 78 s): nothing placed fails to
  decode and every record re-encodes identically. CELL, REFR and ACHR are unordered in the
  files: 784, 1,814,375 and 11,288 subrecords fall outside the best linear member order and
  stay raw in `extra` (gaps row). The full run also asserts the cell/world group layout
  (types 1..9, block and subblock label rules); `docs/schema/world-family.md` has the
  counts. Other lanes' tests touched to stay green (each a few lines): `esm` bridge and
  driver unit tests (CELL/REFR/NAVM now defined), engine `schema` test (REFR -> LAND as the
  undefined type), schema `defs.rs` lists, the golden `seed.rs` (regenerate with
  `KITKRAFT_UPDATE_GOLDEN=1` after merging), and `doc` `typed_corpus.rs` now resolves a
  subrecord's layout by the generated matcher's placement (REFR has two `INAM` layouts,
  and BFCB bodies hold `PNAM`/`NAM*` of other sizes). `crates/esm/src/generated/` is not
  committed: run `cargo xtask codegen` after merging.
- M4.BX1 WORLD merge (integrator, 2026-10-14): the world family's full CELL, LCTN, REFR and
  WRLD win over round 3's lstring-only stubs (every lstring round 3 listed is a member);
  `typed_corpus.rs` keeps main's positional resolution (it supersedes the lane's
  `match_members` placement); `bridge`/`driver`/`schema` tests take "the type is defined"
  (LAND is the undefined type). Shared blocks folded: PositionRotation's position and
  rotation and ObjectBounds' corners are `ref:Vec3`, OwnerData's NoCrime is `ref:Bool8`;
  `common_blocks_are_not_duplicated` (`crates/schema/tests/defs.rs`) fails on two
  `common/` blocks with the same layout or one spelling out another shared type inline.
  `validate_corpus_official_files` explains the 86 new errors as `KNOWN_ERRORS`: object ID
  0x14 in PLYR-capable reference slots (EnableParent x6, XATR x4, XLKR x1, LCTN LCEP x2),
  REFR NAME 0x1FB x1 (Starfield.esm); REFR XPLK stored one file index above the plugin's
  own in SFBGS00D (4), SFBGS050 (4) and ShatteredSpace (30), every target present at the
  plugin's own index; SFBGS050 CELL XBPS snap links to 47 references the file lacks.
  Gates: `cargo xtask ci`, ignored release corpus tests (14/14 identical, typed check 0
  conflicts, `world_family_full` passes, doc `typed_corpus` 7,239,631 typed records);
  parity: 75 record definitions, 41,197,795 typed, 2,035,002 opaque, 7,186,764 unknown;
  **1,712,200 failed**, all subrecords the ordered matcher cannot place in unordered CELL
  (784), REFR (1,700,128) and ACHR (11,288) records (kept raw in `extra`, lossless; the
  gaps row), none a payload that failed to decode. They go to 0 with lane B's `unordered`
  record mode (Requests).
- M4.BX1 lane A M4 merge (integrator, 2026-10-14): `esm::cells`, the rebuild gate, the
  forms allocator, NAVM import, `refs::compose_reference`, `upsert_styled`; `bridge` keeps
  one implementation (lane A's: `check_subrecords` reads `Record::extra()`, no `extra_of`)
  with its all-types test. With the world family on the branch, `compose_reference` takes
  REFR/ACHR `DATA` Z and rotation at `corpus` (gaps row updated; flag 0x800 still needs
  `allow_reference`). Ignored release tests: `corpus_rebuild_full` 0 differences on all 14
  files in both orders (the 12 official-pipeline files whole and byte-identical;
  BlueprintShips-Starfield 65 and BlueprintShips-SFBGS050 4 listed normalizations),
  roundtrip 14/14, typed check 0 conflicts; parity unchanged.
- M4.BX1 later merges (integrator, 2026-10-14): the policy docs (ADR 0007 amendment, 0008,
  0009), lane F (bare-name resolver, `roundtrip --decode`), main (owner's brief, live-game
  rules), lane E M4.E11 (`deploy`), lane G M4 (`.psc` reader, `papyrus.events`,
  `package`). `docs/source-registry.md`: the policy branch's COMMONLIB/SFSE rows, lane G's
  PAPYRUS/OPEN-PAPYRUS rows. End-to-end on a synthetic MO2 instance in a temp dir (`set`
  rename -> `package` -> `deploy --dev` -> redeploy -> `deploy.rollback` ->
  `deploy.status`): the package manifest verified, rollback restored the first plugin byte
  for byte, profiles and `ModOrganizer.ini` untouched; one mismatch fixed (`package`
  writes `plugins` as `{file, ...}` objects, which `deploy` now reads). ADR 0009
  Amendment 1 accepts the five departures `docs/deploy.md` lists.
- `deploy --dev` / `package` fix round after a Fable review (2026-10-14): the `Default`
  gate fails closed (UTF-8 BOM stripped in `Ini::parse`; no `selected_profile` refuses
  `live-profile-unknown` unless `--profile` names an existing non-Default profile;
  diagnostics read only an accepted `profile`); `patch::absolute` resolves through the
  deepest existing ancestor and `guard_location` refuses any link or junction (`set`,
  `build`, `package`); `.previous`/`.retired` are dropped only when they match their
  manifest (`previous-modified`); `mods\` is checked against the user Data folder and may
  not be overwrite, downloads or profiles; `deploy.status` reports one odd folder as
  `state: "error"`; a `<target>.kitkraft-lock` (`create_new`) serializes writers and
  guards recovery; path length is measured under `.previous` and the longest temp name;
  rollback copies `meta.ini` after the swap and recovery removes our `.tmp` names;
  `package` writes archive entries' `source_sha256`, replaces by renames only, and shares
  the 20,000-file limit with `deploy`. `docs/deploy.md` gains operating notes.
- **M4.E11 `deploy --dev` (lane E, ADR 0009, 2026-10-14).** Commands `deploy` (CLI
  `kit deploy --dev <source> <mo2>`), `deploy.rollback`, `deploy.status` in
  `crates/engine/src/deploy/` and `handlers/deploy.rs`; `docs/deploy.md` has the shapes,
  layout, algorithm, refusal rules and a read-only `--dry-run` against the owner's
  instance (refused: MO2's active profile is `Default`; nothing written). A plugin or
  package folder goes into `<mods>\KitKraft Dev - <stem>\` only: `kitkraft-deploy.json`
  (marker, SHA-256 and source of every file, time, previous id), staged in
  `<target>.staging`, swapped in by folder renames, the replaced one kept as
  `<target>.previous` for rollback, a journal beside them recovered on the next call;
  refuses `Default` (active or `--profile`), a same-named folder without the marker, hand
  edits, links/junctions; never enables the mod (the report lists the MO2 steps). Tests on
  a synthetic MO2 instance (14): first deploy, redeploy, rollback and its undo, Default
  refusal, collisions, interrupted deploy at four points and interrupted rollback
  recovered, hand edits, a junction, package-manifest verification, profile files,
  `ModOrganizer.ini`, an unrelated mod, `overwrite\` and `downloads\` byte-identical.
  Differences from ADR 0009's text (per the owner's brief) are listed in
  `docs/deploy.md`; the ADR is on an unmerged branch and needs an amendment.
- Lane E, M4 (set + validate + build, 2026-10-14):
  - M3.E1 (carried from M3): `set` and `build` no longer refuse a whole load order. Each
    copied form goes through lane D's `Loaded::source_check` (`handlers/set.rs`
    `safe_source`): a blocking problem (a holder of a version loading before its master, a
    later plugin that failed or has unresolved records that could be the form, a missing or
    deleted winner) refuses that form; every other problem (an active plugin with no file,
    an unrelated ordering error) is a limitation. Tests through `execute`: a synthetic MO2
    profile with an active plugin that has no file writes an unrelated form with a
    limitation and refuses a form a plugin needing the missing one may override.
  - M3.E2: lstrings inlined from string tables are the table's bytes (`Resolved::bytes`),
    so the few non-UTF-8 official entries are copied exactly instead of refused;
    `record.flags-body` accepts any one NUL-terminated string. The `docs/gaps.md` row is
    closed.
  - M4.E2 `place` (`Implemented`; `handlers/place.rs`, writer `edit::patch::write_patch_with`;
    CLI `kit place <base> --position "[x,y,z]" <into> --cell <CELL>` or `--world <WRLD>`):
    a new `REFR` (`ACHR` for an `NPC_` base) composed by lane A's `compose_reference`, a
    FormID from `NewForms` once the master list is final, `HEDR` next object ID advanced;
    the winning `CELL` (and `WRLD`) carried as an unchanged copy before the children groups
    (a cell the target already has is kept where it is, and the reference goes under its
    observed path); `world` + position picks the exterior cell by `XCLC` grid, and a
    persistent reference of a world goes into the world's persistent cell. Typed refusals
    (`place.<code>:`): base not placeable (references, cells, worlds, navmeshes; `LVLN`
    until checked), no cell or world, cell and world both, no exterior cell at the grid,
    a temporary reference in a persistent cell, EditorID invalid or clashing (load order,
    target, or twice), a small or medium master (`place.scaled-master`, per lane A's review
    round), a parent not carried, a FormID already in the target; a position outside the
    named exterior cell is a limitation. Evidence: base and cell winners, every written
    record; `placed[]` lists base, cell, world, grid, group path and each member with its
    confidence; `carried[]` says whether each carried record equals its winner byte for
    byte. Tests through `execute` (synthetic master: interior, persistent and exterior
    cells; ACHR; observed path; determinism and the CLI form; every refusal) and
    `tests/place_corpus.rs` (ignored, release, 18.6 s): 8 interior and 8 exterior
    `Starfield.esm` references with only `NAME`+`DATA` placed again from their base, cell
    or world, position and rotation reproduce their record bodies byte for byte
    (header: new FormID, composition-default stamps).
  - M4.E3 `build` spec version 3 (`build/spec.rs`, `handlers/build.rs`): `[[place]]`
    entries (as `kit place`, with an optional spec-local `name`; positions and rotations as
    three finite numbers), `[[import]]` (every `NAVM` of a Creation Kit plugin copied
    unchanged by lane A's `import_navmeshes`; refused unless the master lists match; `ONAM`
    redone and every written record located again), class rules over typed world records
    (a rule over `REFR` writes overrides with their cells carried). The writer now writes
    `TES4` `ONAM` by lane A's M4.A6 rule (overrides of master records in groups 7, 9, 10, in
    file order; F23 open) for every `set`, `build` and `place`. Each cell a place needs is
    carried once, and not when a rule or set writes it. `plan` lists the new forms with
    their FormIDs; versions 1 and 2 build as before. `[[new]]` and `[[clone]]` landed with
    M4.E1 (below); a `new:` name of a placed reference is refused, since a placed
    reference is never a base, cell or world. Tests through
    `execute` (`handlers/build_v3_tests.rs`): two builds byte-identical, the plan, rules over
    `REFR` with `ONAM`, an import (and its master refusal), and an error per case (unknown
    name, a cycle, a placed reference as a base, duplicate names and EditorIDs, positions,
    a `reference`-only member without `allow_reference`, two writes to one field,
    `[[new]]`, version 2 keys).
  - M4.E4 validate cell rules (`validate/cells.rs`; rows in `docs/validate.md`):
    `cell.missing-parent` (a cell child outside cell children, or a WRLD/CELL children
    group whose record nobody has: error; a master's record not carried: warning),
    `ref.orphan` (a REFR/ACHR base that resolves nowhere, or no NAME: error),
    `ref.outside-block` (a cell in the wrong block/sub-block for its object ID or XCLC grid:
    error, warning in small/medium plugins; an exterior reference outside its cell's
    square: warning), `ref.persistent-flag` (flag 0x400 against group type 8/9, on a NAVM,
    on a CELL that is not a world's persistent cell: error), `ref.persistence` (a temporary
    reference a quest, scene, package, dialogue or story-manager record or a door link
    names: warning, grade D), `cell.actor-without-navmesh` (warning; masters scanned by
    group headers only), `cell.incc` (error; missing INCC with interior cells: warning),
    `navmesh.unchanged` (a NAVM override equal to its master's: warning). Positive and
    negative synthetic fixtures per rule (`validate/cells_tests.rs`).
    `validate_corpus_official_files` (release, 279 s): 0 unexplained errors; one new known
    error (`ref.orphan` on `Starfield.esm` `REFR:00014893`, base 0x1FB, the same record
    `formid.dangling` lists). New warnings by kind, with their reasons:
    `ref.outside-block` REFR (Starfield.esm 4, SFBGS00D 57, SFBGS050 217, ShatteredSpace
    76: exterior references outside their cell's square, the corpus's 0.02 %);
    `navmesh.unchanged` NAVM (SFBGS003 301: navmesh overrides byte-equal to Starfield.esm's,
    an official-pipeline habit, so a warning); `ref.persistence` REFR (SFBGS00D 5: door
    links, `XTEL`, to temporary references). Corpus finding that shaped `ref.persistence`:
    official files name temporary references from `WRLD` `RNAM` and `LCTN` `LCSR`/`LCPR`
    (48,538 in Starfield.esm), so those referrers are not reported.
  - M4.E6 `testplan` (`Implemented`; `handlers/testplan.rs`; CLI `kit testplan <plugin>
    <out> --profile <dir>`): writes only `out/kitkraft-test.txt` (CRLF console commands:
    `coc <cell EditorID>`, `player.moveto`/`prid <runtime ID>` per new reference,
    `player.additem <runtime ID> 1` per new item, `coc`+`prid` per reference override) and
    `out/checklist.md` (setup on the MO2 profile `KitKraft Dev` from a disposable save, the
    plugin's SHA-256, a step per form with the expected observation, persistence steps,
    the `docs/game-observations.md` ledger entry, the removal rule for plugins that own
    forms). Runtime IDs from lane D's `LoadOrder::runtime_id` with the plugin appended (a
    master-flagged plugin after the last master-flagged one): assumptions
    `runtime-slots-by-scale` and `masters-load-first` and the batch folder (grade D) are
    limitations. Deterministic; `out` guarded (never game/user Data or MO2). Tests through
    `execute` (`handlers/testplan_tests.rs`).
  - M4.E1 `new` and `clone` (`Implemented`; `handlers/newform.rs`; CLI `kit new <type>
    <edid> <into> [--fields JSON] [--masters ...]`, `kit clone <form> <edid> <into>
    [--fields JSON]`): a new base form of a typed top-level type (`EDID`, the schema's
    required top-level members as zeros or an empty string, each field's subrecord created
    in schema order then written as `set` writes it), or a copy of a top-level winner with
    a new FormID and EditorID (lstrings inlined, FormIDs as stored, remapped or refused as
    the patch writer does). FormIDs from lane A's `NewForms` after the master list is final
    (`write_patch` `Copy::new_form`); full-scale targets and masters only; EditorID clashes,
    nested types (references: `place`; cells, worlds, quest children), untyped types,
    `EDID` in fields and repeated occurrences are typed refusals (`new.<code>`,
    `clone.<code>`). Spec v3 `[[new]]` and `[[clone]]` (names; a `[[place]]` base may be
    `new:<name>`; a clone of a form of the same build and a new cell or world are refused).
    Tests through `execute` (`handlers/newform_tests.rs`, `build_v3_tests.rs`). **W1
    deliverable** on the official file: `kit new GLOB KitKraftTestGlobal` (FLTV 1.0) and
    `kit clone Starfield.esm:1E4FBD KitKraftCrateClone` into `out\w1\KitKraftW1.esm`
    (430 bytes, SHA-256 `5d122dd3c93a0fecdbe8cde6da116f5f9c710d45d49d07a559ae51e68c2c33ae`):
    FormIDs `01001000`, `01001001`, `HEDR` next object ID `FF001002`; validate 0 findings,
    roundtrip identical, `inspect` finds the clone. Whether they equal the CK's choice is
    fixture F06's question (not observed; named in `limitations`).
  - M4.E8 recipes `docs/recipes/ownership.md` + `ownership.toml` (W4: `XOWN.Owner` of
    `CityNewAtlantisFreestarEmbassy` set to `PlayerFaction`; 15,053 bytes, SHA-256
    `cc77ed38...cc259`) and `docs/recipes/actors.md` + `actors.toml` (W9: an `ACHR` of
    `Vendor_ChunksEmployee_M` in the embassy, which has a `NAVM`; 15,190 bytes, SHA-256
    `837b6371...8f27a`): each builds, validates with 0 findings and roundtrips identical;
    every fact cites its source. `cell.actor-without-navmesh` caught the first choice of
    cell (the POI's apartment has no `NAVM`). Finding: `query` by owner cannot decide 65
    `CELL`s (`XOWN` out of the ordered definition's place; lane B's `unordered` mode).
    The ownership cell is a stand-in until M4.H1 (F27). In-game steps for M4.O4 in each
    recipe (KitKraft Dev, disposable save).
  - Not started, gated: M4.E9 (script attachment) waits for BQ's typed script-attachment
    block (`VMAD` is opaque in every definition); M4.E10 (procedural placement) waits for
    M4.D3 (`planet`, not on main).
  - **First POI (rung 0) built** (`docs/poi/first-poi.md`, spec `docs/poi/first-poi.toml`):
    `out\poi\KitKraftFirstPOI.esm` (1,821 bytes, SHA-256
    `13c22b482dd0067a741ebd2d2d57a4824623253492cbe38833768cfa3757b5f1`, master
    `Starfield.esm` only): one persistent `REFR` `KitKraftFirstPOICrate` (`01001000`) of
    `Crate_MilitarySupply_01` (`Starfield.esm:1E4FBD`) in the interior cell
    `CityNewAtlantisApartmentTEMPLATE` (`Starfield.esm:21966F`, one version on Default).
    Two builds identical; `validate` 0 errors 0 warnings (14 rules); `roundtrip --verify`
    identical; `inspect` with the plugin appended finds it; `testplan` against the Default
    profile (read only) gives runtime ID `28001000`; `package` validated it into
    `out\poi\package`. **Owner (M4.O2):** on **KitKraft Dev** with a disposable save,
    `coc CityNewAtlantisApartmentTEMPLATE`, then `player.moveto 28001000` (or
    `player.moveto KitKraftFirstPOICrate`), `prid` it, screenshot, and run the persistence
    steps in `docs/poi/first-poi.md`.
- Lane G, M4 (archives, scripts, strings; 2026-10-14):
  - M4.G0: ADR 0006 Amendment 1: `.psc` declaration facts come from the official sources
    in `Tools\ContentResources.zip` read in place (level 1), the compiler as a black box
    (level 2) and the CK wiki for semantics (level 4); the Papyrus Index only cross-checks;
    no other Papyrus parser, compiler or grammar is read; source comments are shown only on
    request and never cached or quoted. Registry rows PAPYRUS, CORPUS-ARCHIVES and
    OPEN-PAPYRUS updated.
  - M4.G1: `kitkraft_pex::psc::parse` reads a `.psc`'s declarations (header with parent
    and flags; functions and events with typed parameters, defaults, return types, flags
    and state; remote event handlers; properties with groups, `RequiresGuard(...)` and
    full Get/Set forms; structs and members; variables; imports, custom events, guards,
    states, groups); bodies skipped; a statement it does not understand is a diagnostic
    with its line, a structural fault a typed `PscError` with its line. Synthetic fixtures
    per construct, truncation and noise never panic. Corpus (`every_official_source_parses`,
    the zip read in memory, 1.5 s debug): all 5,033 `.psc` files parse with 0 errors and
    0 unread statements: 21,175 functions (1,223 native), 3,567 events (515 remote
    handlers), 27,000 properties, 167 structs, 58 custom events.
  - M4.G2: `papyrus.events` (`Implemented`; CLI `kit papyrus events --script
    ObjectReference`, `--event OnLoad --declarations-only`, `--search outpost`; MCP via
    `execute`/`command_line`): the zip read in place, index cached by its central
    directory (no comments in the cache), parent chains resolved, overrides and remote
    handlers marked, `with_comments` reads doc/comment from the user's zip at query time,
    `file:line` plus byte-range evidence. Counts per kind on the official zip (5,033
    scripts): event 3,567, function 21,175, property 27,000, struct 167, custom_event 58;
    `OnLoad` is declared (not overridden) in ActiveMagicEffect, ObjectReference,
    RefCollectionAlias and ReferenceAlias; ObjectReference's chain is Form, ScriptObject;
    `--search outpost` 491 matches. Tests: synthetic zip (chain, overrides, remote
    handlers, search, paging, comments only on request, cache hit/miss/corrupt, errors,
    command-line forms) and `official_sources`. **Open:** the acceptance check that the
    events `docs/design/persistence.md` cites exist waits for that file (M4.H3, lane H; not
    on main): `kit papyrus events --event <name> --declarations-only` is the check.
  - M4.G3 (scoping): `docs/research/asset-formats.md`: `ba2.list` over all 90 official
    archives (1,763,906 entries, 138.5 GiB; 65 GNRL v2, 10 DX10 v2, 15 DX10 v3 LZ4) by
    extension (38 extensions) and by archive group, with what KitKraft does per format,
    the permitted source if any (only DDS headers, string tables and `.pex` have one) and
    the workstream that would author it. Finding: no `.mat` file exists in any official
    archive; materials ship as a `.cdb` component database. No implementation.
  - M4.G4 (research): `docs/corpus.md` "Archive names": all 90 archives are `<stem> -
    <suffix>.ba2`; the 50 base-game archives (and `BlueprintShips-Starfield -
    Localization`) are named in `Starfield.ini` lists; the other 38 belong to 11 official
    plugins by name only (suffixes `Main`, `Main01/02`, `Textures`, `Localization`,
    `Voices_<lang>`). Rule for `package`: `<plugin stem> - Main.ba2` (GNRL) and `<plugin
    stem> - Textures.ba2` (DX10). Grade C until M4.O2 sees a mod's `- Main.ba2` load.
  - `package` (the plan's M4.E5 shape, assigned to lane G for M4; `Implemented`; CLI `kit
    package <plugin> <out>`; MCP via `execute`/`command_line`): `validate` first (a failing
    plugin is refused, nothing written); `<out>\<name>\` with the plugin, loose files and
    scripts (`Scripts\<namespace>\<name>.pex` from the name inside each `.pex`) at their
    Data paths or packed per M4.G4; `kitkraft-manifest.json` (SHA-256 and size of every
    file, masters, record counts per type, KitKraft version, spec SHA-256, validate
    result, archive contents); a deterministic zip; only `out` is written, and an `out` in
    Data, the user Data dir or MO2 (its `mods\` included) is refused. Deviation from the
    plan's arguments: one `plugin` (not `plugins`), since `plugins` names the load order
    in every other command. Tests: two runs byte-identical (folder and zip), the manifest
    verifies after a copy, ba2 names and contents, validate refusal, every guard,
    command-line form. Real runs 2026-10-14: the rename plugin packaged (manifest SHA-256
    equals `docs/smoke-rename.md`); `black-box: Archive2.exe 1.2.0.1` extracts a `--ba2`
    package's two archives with the same file list (general files identical, texture
    pixels identical under Archive2's own DDS header). Install steps and the manifest check
    in `docs/packaging.md`. **Owner (M4.O2):** install the rename package through MO2 on
    `KitKraft Dev` and check the hashes on disk against the manifest.
- M4.BC1 ACTOR schema family (family lane BC, `crates/schema/defs`, 2026-10-14): NPC_,
  LVLN, OTFT, CLAS, AFFE, RACE, PERK, SPEL, MGEF transcribed from the pinned xEdit files
  (one commit per type, every member cited with its line range); new shared `common/`
  definitions `SoundReference`, `AttackData`, `Effect`. Corpus check (14 files, generated
  decoder regenerated in the worktree): all 13,350 records of the 9 types (NPC_ 8,447, all
  compressed; LVLN 298; OTFT 698; CLAS 8; AFFE 1,593; RACE 47; PERK 608; SPEL 662; MGEF
  989) decode with every subrecord placed, nothing in `extra`, and re-encode
  byte-identically; `corpus_full_roundtrip` (release) 14/14 identical, 0 typed conflicts;
  doc `typed_corpus` all files passes (60,063 typed records, the 181 pinned unresolved
  lstrings as before). Confidence `corpus`, members never seen in an official file
  `reference`. Opaque by design: VMAD, component blocks, NPC_ ACBS level union, NAM5, OBTS,
  ONA2 tail, RACE DAT2 unnamed words, PERK effect DATA/EPFD unions, ATAV, MGEF archetype
  slots (gaps rows in `docs/schema/actor-family.md`, the mod-author guide). `cargo xtask
  ci` green in the worktree with the generated files regenerated; generated output, the
  `BUILTIN_DEFINITIONS` region, the golden `seed.rs` and the `tests/defs.rs` lists are left
  to the integrator (9 records, 3 common files to add).
- M4.BX1 ACTOR merge (integrator, BX3, 2026-10-14): the actor family's full CLAS, MGEF,
  NPC_, PERK, RACE and SPEL win over round 3's lstring-only stubs (every corpus-table
  lstring is a member). Shared blocks folded: the family's `SoundReference` (an event-set
  struct plus the start form, the same 40 bytes) into the existing flat one, LVLN's and
  LVLP's inline model groups into `common/Model` (same members and cardinalities), NPC_'s
  opaque destructible block into `common/Destructible` (absent from the official NPC_);
  `common_blocks_are_not_duplicated` now also checks that those records reference the
  shared blocks. Derived files regenerated (83 record definitions, 32 shared).
  **Engine-reserved object IDs** (ADR 0005 Amendment 4, `docs/validate.md`): instead of
  listing the 318 new `PRPS.ActorValue` errors, `formid.dangling` no longer reports a typed
  member naming an object ID below 0x800 in `Starfield.esm`'s index space that it does not
  define; it counts them (`reserved_form_ids`, `reserved_by_member`, a limitation). Every
  earlier `< 0x800` known error (PKIN/FLST actor values, the player reference 0x14, REFR
  NAME 0x1FB, SMEN PNAM 0x5B, quest alias ALFR 0x14) left `KNOWN_ERRORS`; the corpus test
  pins the counts per plugin and member path instead (`RESERVED`: 738 in all, 397 in
  Starfield.esm: NPC_ PRPS 139, RACE PRPS 178, FLST LNAM 23, PKIN PRPS 21, SMEN PNAM 20,
  ...); the DLC warnings of the same class are gone. The one other new error,
  `NPC_:0018B5F4` `RDSA.SpecificReference = 00577823` (a stale value its flags do not
  enable), is a `KNOWN_ERRORS` entry. Synthetic test `formid_dangling_skips_engine_reserved_ids`.
  Gates: `cargo xtask ci`; ignored release tests for esm, doc and engine (31 min; doc
  `typed_corpus` 7,257,137 typed records, 181 pinned unresolved lstrings). Parity: 83 record
  definitions, 45,923,589 typed (1,712,200 failed, the unordered CELL/REFR/ACHR placements),
  2,262,151 opaque, 2,233,821 unknown.
- M4.BI1 ITEMS/WORKSHOP schema family (parallel schema lane, `crates/schema/defs`,
  2026-10-14): STAT, MISC, CONT, ACTI, FURN, MSTT, TERM, TMLM, ARMO, IDLE, ANIO, CPTH
  transcribed from the pinned xEdit files (staged copies verified first: sizes and blob
  hashes match `docs/xedit-reference.md`; one commit per type, every member cited with its
  line range). New shared `common/` definitions: Model (first owner, as planned),
  Destructible, ContainerItems, ObjectTemplate, SoundReference, ObjectProperty, ColorRgb,
  RadioReceiver, FurnitureMarkerFlags, FurnitureMarkerParameter, FurnitureEntryPoint
  (ObjectBounds already existed from the planet family and is reused). Corpus check (14
  files, out-of-tree checker over the generated decoder): all 33,939 records of the 12
  types (STAT 22,296, MSTT 2,134, ACTI 1,940, MISC 1,495, IDLE 1,302, ARMO 1,168, FURN
  1,045, TMLM 974, CONT 866, TERM 501, CPTH 152, ANIO 66; none compressed) decode with
  every subrecord placed, nothing in `extra`, and re-encode byte-identically; confidence
  `corpus`, members never seen `reference`. Opaque by design: VMAD, component blocks,
  NVNM, CTDA unions, COED's middle slot, OBTS, FURN NNAM, TERM WBDT, TMLM padding (gaps
  rows in the guide). Lstring members equal the corpus lstring table for these types.
  Mod-author guide with the outpost build menu read from the corpus (recipe categories,
  beacon keywords, created-object traits, what a new buildable needs):
  `docs/schema/items-workshop-family.md`. One small change outside the lane, in its own
  commit: two lane E unit tests used ACTI ATTX as their example of an lstring the schema
  does not type; now typed, the examples move to FLOR ATTX (Requests). Derived files left
  to the integrator (family lane rule): `crates/esm/src/generated/`, the
  `BUILTIN_DEFINITIONS` region (12 records, 11 shared files), `crates/schema/tests/golden/
  seed.rs` and the builtin lists in `crates/schema/tests/defs.rs`; with them regenerated
  locally `cargo xtask ci` passed (with `KITKRAFT_DATA_DIR`).
- M4.BX1 ITEMS merge (integrator, BX3, 2026-10-14): the items/workshop family's full ACTI,
  CONT, FURN, IDLE, MISC, MSTT, STAT, TERM and TMLM win over round 3's lstring-only stubs
  (every corpus-table lstring is a member); lane E's two tests keep main's ACTI ATTX example
  in `edit::fields` (typed, no fallback) and take the lane's FLOR ATTX one in `patch_tests`.
  Shared blocks folded (one definition each, every citation kept, every reference
  updated): the family's `Model` into main's (MOLM becomes `repeated`: one STAT has two),
  `Destructible`, `SoundReference` and `ColorRgb` into main's; its `ObjectProperty` into the
  world family's identical `PropertyValue`, which PKIN, NPC_, RACE and CLAS PRPS now use too
  (they spelled the element out inline); its `ObjectTemplate` and the inline object
  templates of NPC_ and GBFM into round 3's `ObteBlock` (FULL made optional: no official
  NPC_ or GBFM combination has one; ARMO, CONT and TERM drop their own STOP for the block's;
  GBFM keeps its second STOP), so OBTS is typed in ARMO, NPC_ and GBFM with 0 new decode
  failures; NPC_'s inline container items and the quest alias inventory into the family's
  `ContainerItems` (the base-object target list from the actor family);
  `RadioReceiver` uses `ref:Bool8`. 86 record definitions, 37 shared;
  `common_blocks_are_not_duplicated` checks every fold. Engine-reserved IDs: the 646 new
  Starfield.esm PRPS actor values (ACTI 128, CONT 443, FURN 13, STAT 61, TERM 1) are counted,
  not reported; `reserved_by_member` is keyed by record type and member path (shared types
  share a path), and `RESERVED` pins 1,410 occurrences in 7 files (Starfield.esm 1,043).
  Gates: `cargo xtask ci`; ignored release tests for esm, doc and engine (17 min; doc `typed_corpus` 7,258,523 typed records, 181 pinned unresolved lstrings); parity:
  86 record definitions, 46,249,514 typed (1,712,200 failed, the unordered CELL/REFR/ACHR
  placements only), 2,261,438 opaque, 1,908,609 unknown.
- Lane E, M3.E9 (first real overhaul build), 2026-10-14: `docs/overhaul/overhaul.toml`
  translates the owner's brief (B1..B14) into 16 classes and 16 rules, each naming its
  row; targets resolved by EditorID/FULL on the `KitKraft Dev` profile (14 WEAP, 4 AMDL,
  3 AMMO, 2 ammo LVLI, 10 OMOD; FormIDs in the spec header). `kit build --plan` now prints
  every change with its brief row, rule, before and after (and each JSON change has
  `brief`); `docs/overhaul/build-1-plan.md` is that output: 42 field changes on 17
  overrides (13 WEAP, 4 AMDL). `out\overhaul\KitKraftGunOverhaul.esm` (43,955 bytes,
  SHA-256 `a44d49bb...4a95342aae`; masters `Starfield.esm`, `OldSovietMarksmanRifle.esm`,
  the rifle's winner in `KitKraft Dev`): validate 0 errors 0 warnings, `roundtrip
  --verify` identical, packaged (`out\overhaul\package\KitKraftGunOverhaul-0.1.0.zip`),
  `deploy --dev --dry-run` refused only by the live-profile guard (MO2's active profile is
  `Default`). Rows: B1, B2, B4, B5, B7, B9, B13, B14 implemented; B3, B6, B11, B12 partial;
  B8 open (the automatic receivers carry no damage property: owner to restate), B10 open
  (OMOD value). The open parts are OMOD property values (`WSPS` on default receivers,
  `ACPT` on magazines), opaque until lane B types `ObjectModProperty.Value1` (Requests).
  `crates/engine/tests/overhaul_build.rs` builds the spec on a fixture of the 33 real
  records (copied from `Starfield.esm` into a temp localized master with the English
  tables; skipped without `KITKRAFT_DATA_DIR`) and checks all 42 changes. Report, owner
  steps and checklist: `docs/overhaul/build-1.md`. **Owner (M3.O5):** the in-game check
  there, on `KitKraft Dev` with a disposable save.
- **KitKraft identifier rename done** (M4.BX1 integration, 2026-10-14; owner decision 1):
  packages `kitkraft-*`, crate paths `kitkraft_*` (every `use`, Cargo dependency, xtask
  `LAYER_TABLE` and layer rule), env vars `KITKRAFT_*`, MCP server name `kitkraft` and
  resource URIs `kitkraft://`, the index cache `%LOCALAPPDATA%\kitkraft\cache` (not
  migrated: rebuilt on first use), the package manifest `kitkraft-manifest.json`,
  `.mcp.json.example`, `.githooks`, docs and plan prose, path mentions
  `C:\Modding\kitkraft` (the director moves the folder after merge), generated code and
  goldens regenerated. Deprecated for one release: the old `KITCRAFT_*` env vars are still
  read when the new one is unset (engine and xtask `env_var_os`, the doc cache, the corpus
  tests), and `deploy` still reads a package's `kitcraft-manifest.json` (`package`
  `overwrite` still recognizes a folder holding one); see `docs/corpus.md`, "Environment".
  The binary stays `kit`; commit history and the remote are unchanged.
- Lane B round 4 (2026-10-15): struct-field unions decided by a sibling field (and an
  optional secondary sibling), applied to `common/ObjectModProperty.toml`: OMOD and OBTS
  property values are typed (8,669 entries, 0 unknown value types, byte-identical; member
  paths `DATA.Properties[n].Value1.Float` etc. for lane E). Unordered-record ambiguity lint;
  COBJ FULL `reference` plus a parity check that `corpus` members occur; coverage counts
  confidence by the placed member (reference decoded occurrences 45,265 -> 1,527); PropertyValue
  `since = 152`, REFR XLOC 9-byte form, INFO TRDA Emotion as a FormID with 0xFFFFFFFF as none;
  reserved object IDs pinned per member, with a warning outside the named set (one: SFBGS00D
  OMOD 0x3B7). Open: TES4 DATA/INTV (docs/gaps.md), GBFM STOP Marker2 is live (not dropped).
- Lane B round 5 (2026-10-15): ADR 0005 Amendment 5 (`container-reference` on TES4 DATA and
  INTV, no parity exemption list); M3.B6 (KYWD and GMST XALG typed from the pinned xEdit
  file, KYWD FNAM and VMAD opaque); a per-family table at the top of
  `docs/record-parity.md`; codegen leaves line layout to rustfmt (goldens and generated code
  byte-identical); X8, non-UTF-8 strings shown as Windows-1252. Gates: `cargo xtask ci`,
  `codegen --check`, `parity --check`, and the ignored release corpus tests of esm, doc and
  engine (roundtrip identical, typed corpus, `validate_corpus_official_files`) all pass.
- **Lane W, M4.W1..W6 (read-only workbench, 2026-10-15; the owner started it the same
  day).**
  - What exists:
    - `crates/ui-egui` (L5). Its only workspace dependency is `kitkraft-engine`, which the
      new xtask `INTERNAL_ONLY` rule enforces (ADR 0002).
    - `apps/kit`, which builds the `kit-ui` binary.
    - `kit ui` in kit-cli, a launcher like `kit mcp` with no `CommandSpec`.
  - `gate.rs` is the only holder of the engine. Its allowlist is `load`, `inspect`,
    `query`, `diff`, `conflicts`, `validate`, `coverage` and `schema`, plus `roundtrip`
    with `verify` and without `out`. It refuses every other command and alias. Tests check
    the catalog classification, try every refusal, and scan the crate's sources.
  - The engine runs on a worker thread. The panel view-models have no egui types and are
    tested against canned envelopes. Every result shows its `kit` line and MCP call, with
    copy buttons; the line is checked by parsing it back with the engine's parser. Every
    result also shows its limitations, coverage and evidence.
  - Checked read-only on KitKraft Dev with the `dev_profile` test and the screenshot tour
    (`docs/workbench.md`).
  - The dependency list is in ADR 0007. eframe 0.36 is built with `glow` only. No fonts
    are compiled in; the window uses the system fonts.
  - Carried:
    - `build --plan`.
    - `refs` and placement context (M4.D1, M4.D2).
    - The M4.W0 shape tests (lane F).
    - The owner's licence ruling (row 7).
- Lane E, M3.E9 round 2 (overhaul build 2), 2026-10-15: `has_opaque` (engine
  `edit/patch.rs`) now flags only unknown layout (whole opaque payload, rest-of-payload run,
  edid-prefix union); fixed-size untyped struct fields and sibling-decided unions are known
  layout, and `untyped_id_runs` refuses a FormID remap (and validate counts a subrecord as
  unchecked) only when such bytes are 4+ non-zero (every official OMOD's are zero), so OMOD
  overrides copy from localized sources and remap (test
  `omod_overrides_of_localized_sources_copy_and_remap`). Spec: the default receivers'
  `WSPS` scaled with the WEAP rate (B3 `077F13` 7.5 -> 5.625, B6 `057555` 1.0 -> 1.5625,
  B11 `04A750` 16 -> 12), B10 `12D03F` ACPT ADD 5 -> 2 (total 7); owner answers applied
  (B8 dropped in brief.md, C8; B11 base 18 with the drum's +30 untouched, 48; B12 as built,
  Large 14, Extended 10, XL 22). `docs/overhaul/build-2-plan.md`: 46 changes on 21
  overrides (13 WEAP, 4 AMDL, 4 OMOD), master `Starfield.esm` only (the smoke test's mod
  folder is gone from `mods\`). `out\overhaul\KitKraftGunOverhaul.esm` 45,271 bytes,
  SHA-256 `5e2f84e7...148f982d1`; validate 0/0, roundtrip identical, package 0.2.0, deploy
  dry run refused by the live-profile guard (MO2 on `Default`). Rows: 13 implemented, B8
  dropped. Report, owner steps, checklist: `docs/overhaul/build-2.md`. **Owner (M3.O5):**
  the in-game check there, on `KitKraft Dev` with a disposable save.
- Lane D, M4 (`crates/doc`; engine `handlers/{refs,planet,load,inspect,build}.rs`), 2026-10-15:
  - **M4.D1 `refs`** (CLI `kit refs <form> [--depth n] [--incoming]`, alias `useinfo`; MCP
    through `execute`/`command_line`, no shortcut; `writes: false`). `kitkraft_doc::links`:
    every value of a member the schema types as `formid`, with its member path in the
    `set`/`where`/`diff` grammar (`WAM2.Ammunition.AmmoType`, `KWDA.Forms[3]`), the raw
    value, the definition's target types (`target_allowed`), file and body offsets, and a
    status that keeps `resolved`, `null`, `none` (`FFFFFFFF`), `missing`, `engine_reserved`
    (the validate rule, a test keeps the constants equal) and `unresolvable` (with the
    reason) apart; subrecords not scanned (no layout, component blocks, undecodable) and
    `opaque` members are counted in `limitations`. Outgoing: the winner's links, breadth
    first to `depth`. Incoming: `kitkraft_doc::backrefs`, a reverse index per plugin from
    the generated decoder's `form_ids()` (raw values; the target turned into each plugin's
    spellings, `full-style-self` included) with cell-group ranges, cached as `<hash>.kclnk`
    beside the record index (schema fingerprint and record count in the staleness rule);
    referrers in cells come after the others with cell, world and persistence; `scope`,
    `types`, paging by cursor. Times (`docs/load-order.md`, "Reverse links"): on the 84-plugin
    `KitKraft Dev` profile 18.9 s cold, 0.9 s warm (index 0.17 s), 220 MB cache. Corpus
    (`crates/engine/tests/refs_corpus.rs`, release, ignored): the rifle `21BBCD` has 111
    links (76 resolved, 35 null, 16 keywords), `WAM2.Ammunition.AmmoType` ->
    `Ammo9x39mm` `2B559B` -> a PROJ at depth 2; `NA_Hard_RetWallASupport_03` lists its 2
    placed REFRs in world `Starfield.esm:01251B` on the 14 official files.
  - **`follow` replaces lane E's local `via`** (`kitkraft_doc::links::follow(lo, reader,
    schema, form, where-path)`): `build` calls it; a hop now reports `missing` and
    engine-reserved values as unfollowed instead of failing on them. Build tests unchanged.
  - `inspect::find_form` is shared (`refs`, `planet`); `Loaded` keeps the cache directory
    and the reverse index.
  - **M4.D3 `planet`** (CLI `kit planet <form>`, alias `pcm`; MCP through `execute`/
    `command_line`; `writes: false`): for a `PCMT`/`PCBN`/`PCCN`, the tree from the nodes'
    upward links (`PRTN`, siblings ordered by `PRVN`, `order` says how), each node with
    selection fields, keywords, conditions as stored (union fields as hex), content and one
    level of content detail, ancestors, the `DFOB` holding a tree (stored-bytes match), and
    every unresolved or out-of-target link of the tree; for a `PNDT` its worldspaces,
    biomes, star (system ID match) and the content nodes placing its worldspaces with their
    branch chains; `STDT`, `BIOM`, `LCTN`, `WRLD`, `PKIN`, `LVLP` views likewise, with
    keyword condition candidates (raw-byte matches) for biomes and locations. Nothing
    evaluates a condition. On the 14 official files all five trees walk with 0 unresolved
    links (2,518 nodes; content counts of `Starfield.esm` equal planet-family.md's), about
    0.9 s each warm; `crates/engine/tests/planet_corpus.rs` (release, ignored) pins the
    trees, a planet chain (Mars -> Sol, 12 placing nodes) and prints the link table.
    `docs/design/planet-content.md`: link statistics by member, tree depths and sibling
    chains, condition function counts, the two typed paths to placed content (tree and
    biome), every claim graded, open questions for M4.O1/M4.O4.
## In progress

- M4.BX1 lane H merge (integrator, BX3, 2026-10-14): `cargo xtask oracle sf1edit` (xDump64
  harness, M3.H4) and its `docs/oracles.md`, `source-registry`, `xedit-reference` and
  gaps rows; conflicts in `docs/gaps.md` and STATUS kept both sides (the stale "not
  installed" and "not transcribed yet" gaps rows dropped). Rerun on the five small official
  files after the three family merges (14.5 min; the dumper's exception log deleted):
  agreements 191 -> 4,963, gaps 9,672 -> 2,225 occurrences; the 2,675 new disagreements are
  enum, flag, GUID and opaque-field display conversions the comparator does not make, not
  value differences (`docs/oracles.md`, "Rerun after the schema integration"; a comparator
  request to lane H). Gates: `cargo xtask ci`, the ignored release tests for esm, doc and
  engine, parity (unchanged from the ITEMS merge).
- M4.BX1 `unordered` record mode (integrator, BX3, 2026-10-14; closes the world lane's
  request and the gaps row): `unordered = true` on a record definition (loader: records
  only, no top-level `any_until`, `DiskOrder` reserved; `RecordDef::unordered`). Codegen:
  `match_unordered` matches a subrecord from the current member on and, when none accepts
  it, from the first member again (groups still match one run in their own order); the
  record keeps `disk_order` (the member of each typed value in on-disk order) and
  `encode` writes the values back in that order (edits: remaining values keep their order,
  added ones follow member by member; a fresh record is written in definition order), so
  decoding stays lossless and every member keeps its path. Generated `UNORDERED` and
  `match_body` pick the matcher for `bridge`, `coverage` and the world family test;
  `kitkraft_doc::typed::placements` mirrors the rule. Applied to CELL, REFR and ACHR.
  Tests: fixture `UNOR` (any order, `XXXX` kept, edits, a second optional value raw in
  place, random bodies), loader rules, doc placement. Corpus: `world_family_full` (release,
  all 6,719,725 REFR, 40,251 CELL, 14,731 ACHR) places every subrecord the definitions name
  (0 unplaced, 0 failed, 0 re-encode differences, 89 s); parity **failed 1,712,200 -> 0**
  (46,249,514 typed, 2,261,438 opaque, 1,908,609 unknown; no subrecord is left undecodable).
  The newly typed out-of-order CELL XBPS and REFR XPLK expose more of the two known
  dangling classes (`validate_corpus_official_files`, fixed at the main merge): SFBGS050
  XBPS 24/23 -> 119/107, BlueprintShips-SFBGS050 XBPS 10/10 (five cells, 20 of its own
  references it lacks), ShatteredSpace XPLK 30 -> 41 (all at file index 02). Main merged
  afterwards (lane E's M4 commands and cell rules, lane A's placement fixes): `ref.orphan`
  keeps its REFR NAME 0x1FB known error; all gates green on the merged head.
- **M2.F7 (owner check pending).** `.mcp.json.example` and "Use from Claude Code" in
  `docs/mcp.md` are in; an agent cannot restart Claude Code, so the owner runs:
**M2.F7 result (2026-10-14):** PASS. Owner opened a fresh Claude Code session: `kitkraft` connected with exactly 8 tools (execute, command_line, coverage, diff_plugins, inspect_record, query_records, record_schema, roundtrip), the Default profile preloaded, `record_schema WEAP` returned 50 subrecord definitions (6 typed, 44 opaque), `coverage Constellation.esm` returned 57 records / 8 types. Caveat filed as a lane F request below.

  1. `cargo build --release -p kit-cli`
  2. `claude mcp add --scope local kitkraft -- C:\Modding\kitkraft\target\release\kit.exe mcp`
     (or copy `.mcp.json.example` to `.mcp.json`, which is gitignored, and approve it).
  3. New Claude Code session: `/mcp` lists `kitkraft` connected, 8 tools.
  4. Ask it to call `mcp__kitkraft__coverage` on `<Data>\Constellation.esm`, then
     `mcp__kitkraft__execute` with `{command: "roundtrip", args: {plugin, verify: true}}`,
     then `mcp__kitkraft__inspect_record` on a form `coverage` names (`first_record`), then
     `mcp__kitkraft__query_records {type: "WEAP"}` and `mcp__kitkraft__record_schema
     {type: "WEAP"}`; each returns an envelope (`query_records` needs a load order: pass
     `--profile` to `kit mcp`, which now preloads it).
  5. Paste a transcript excerpt here. Headless alternative: `claude -p "..." --mcp-config
     .mcp.json --allowedTools "mcp__kitkraft__*"`.
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

### M3 lane H (evidence: CK observation and oracles)

| Task | State |
| ---- | ----- |
| M3.H1 fixture matrix and session scripts | **done** except F07's recompression candidates (wait for M3.A1's Request): `docs/ck-observations.md` has the ledger format, the matrix F00..F14 with every form resolved (FormID, EditorID, why, the command that found it), the M3.O1 script per fixture, the M3.O2 lists with KitKraft's assumed runtime IDs, and the `fixture.toml` template |
| M3.H2 `cargo xtask observe` | **done**: `cargo xtask observe <fixture-dir>` writes `report.md`/`report.json` in the fixture folder only (SHA-256 of every file; per plugin TES4, `roundtrip --verify`, `coverage`, `validate`; base, `[[compare]]` and `kit_equivalent` comparisons as byte runs, TES4 diff and `kit diff`, our equivalent in both styles with `ck` reported unavailable until M3.E5); `cargo xtask observe new <F-id>` writes the fixture's `fixture.toml`. Tests: synthetic fixture dirs with a fake `kit` (nothing written outside, rerun byte-identical) and the real `kit` when built |
| M3.H4 oracle harness | **Spriggit part done**: `cargo xtask oracle spriggit [--official] [--synthetic] [<plugin>...]` (YAML deleted unread; compared with `kit diff`, `coverage`, `roundtrip`, byte and TES4 diffs); run 2026-10-14 on the 5 smallest official files, 3 hand-built synthetic plugins, 3 `kit set` overrides and the rename plugin; `docs/oracles.md` has the build, fixture set and 11 disagreements (S1: 885 lstring-sized values zeroed in localized files; S3: our TES4 `DATA` after `MAST`, see Requests). M3.O1 fixtures and the overhaul build: rerun when they exist. **SF1Edit part done** (2026-10-14, after M3.O3): `cargo xtask oracle sf1edit [--official] [--synthetic] [<plugin>...] [--types ..] [--sample N]` runs xEdit 4.1.5f's console dumper (`xDump64.exe`; `KITKRAFT_SF1EDIT` = `xSFEdit64.exe` or the dumper) on a scratch Data copy and compares with `kit query`/`inspect`; the editor's script mode is **blocked** headless (Module Selection window; without `-script`, the developer message), documented in `docs/oracles.md`. Results: all records, FormIDs, signatures and EditorIDs agree where xEdit can read the file; 711 decoded members agree on `Starfield.esm` WEAP/AMMO/PROJ/COBJ/OMOD (50 each) and 191 on the four readable small official files (every record), 1 disagreement (X8, ours: lossy cp1252 lstring text); version skew: GPOG/GPOF unknown, SFBGS047 (light master) not loadable, 51 WEAPs with an undecodable component; the coverage-gap list per round 3 type is in `docs/oracles.md` for M3.B11 |
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
| schema     | Round 3: M3.B0-B5, B7, B8 done (see In progress); B6, B9-B12 open                                               | `crates/schema` (lane B)                     |

Lane B round 3 transcribes definitions from the pinned xEdit files (`xedit:` provenance,
`confidence = "reference"` until the corpus confirms) in this priority: **WEAP, AMMO, PROJ,
AMDL, OMOD, COBJ, ZOOM**; then **GMST, GLOB, KYWD** typed; then **LVLI, FLST, INNR**. Each
definition is checked against every corpus occurrence before it moves to
`confidence = "corpus"`; unions stay opaque.

## Blockers

- None for lane B (the pinned xEdit files are staged, M3.O0).

## Decision records (owner, 2026-10-14)

- **M2 fully closed.** M2.F7 (fresh-session MCP check) and M2.O6 (in-game rename) both
  passed (results under "In progress" and "Done" above); no M2 task remains open.
- **M3.O3 done** (was M2.O3): xEdit is installed at `C:\Modding\tools\xEdit\`; Starfield
  mode is `xSFEdit64.exe` (file version 4.1.5.0, older than the `dev-4.1.6` pin
  `9fb01688...`, so M3.B11 records the skew). Unblocks M3.H4's SF1Edit part
  (`KITKRAFT_SF1EDIT`) and M3.B11 (see Requests).
- **Owner decision 1 closed: the name is KitKraft.** Identifiers are renamed too:
  crates `kitkraft-*`, env vars `KITKRAFT_*` (`DATA_DIR`, `CACHE_DIR`, `SF1EDIT`), MCP
  server name `kitkraft`, resource URIs `kitkraft://`, the cache dir, and the repo folder
  `C:\Modding\kitkraft`; the binary stays `kit`. **Rename done** in the M4.BX1
  integration (see "Done"); the old `KITCRAFT_*` env vars and `kitcraft-manifest.json` are
  read as a deprecated fallback for one release. Prose uses KitKraft.
- **M4.O3 decided, default overturned** (ADR 0008): SFSE, native runtime extensions and
  analogous frameworks are supported where technically justified; core crates never
  depend on them (`runtime/` or an `RT` crate); CommonLibSF not read or linked until its
  license exceptions are quoted from its `LICENSE` and reviewed; SFSE binaries never
  bundled; game versions tracked (owner's install: `Starfield.exe` 1.16.244.0 Steam, SFSE
  0.2.21); PC-first; gameplay is not compromised only to avoid SFSE. Policy, not an
  implementation order: no runtime component in M4.
- **M4.O5 decided, default approved with an exception** (ADR 0009): `package` stays
  target-independent and never writes into MO2; a separate, explicitly invoked
  `kit deploy --dev` (M4.E11, lane E, later) writes one isolated
  `KitKraft Dev - <plugin>` mod under MO2's `mods\` with path validation, a hashed
  manifest, temp + rename replacement, diagnostics and rollback; never profiles,
  `plugins.txt`, `loadorder.txt` or other mods.
- **M3.O6 decided: ADR 0007 accepted with scope** (Amendment 1): the egui M5 deferral is
  partially overturned. A minimal read-only developer workbench (L5 `crates/ui-egui`,
  `apps/kit`, over `Engine::execute` only: record browser, structural diff, schema and
  provenance, FormID and reference diagnostics, build validation) is planned as M4 lane W
  (M4.W0..W6) after the inspection APIs stabilize; the full authoring UI stays M5. M3.F5
  is not built; M4.F3 is superseded.
- **CK sessions deferred** (M3.O1 and M4.O1, F00..F28): until the developer workbench
  exists (M4.W2..W5), so the owner can A/B the CK against KitKraft's inspection surface;
  computer use is available for an agent-driven CK session then. The first POI path does
  not wait; `ck-diff` confidence, the `ck` write style and the CK fixture questions do
  (`plan/execution-plan.md`, M3 and M4 "Ordering" updates).

## Open owner decisions

| #   | Decision                                | Default until decided             |
| --- | --------------------------------------- | --------------------------------- |
| 1   | Project name                            | **decided 2026-10-14**: KitKraft; identifiers renamed to `kitkraft-*` / `KITKRAFT_*` / `kitkraft://` (**done**, M4.BX1 integration; old env names read for one release); binary `kit` |
| 2   | xEdit MPL source policy (ADR 0004)      | **decided 2026-10-14** (pinned xEdit SF1 definitions as reference) |
| 2a  | Review `docs/xedit-reference.md` and ADRs 0004/0005/0001 before lane B round 3 | **approved 2026-10-14** (M2.O1; lane B unpaused) |
| 2b  | Whether `wbDefinitionsCommon.pas` (helpers the SF1 file uses) may be cited too | **approved 2026-10-14**: yes, same SHA, same rules (ADR 0004 owner approval) |
| 3   | Video critique used as overhaul brief   | none; overhaul work is after M2   |
| 4   | License                                 | MIT OR Apache-2.0 (scaffolded)    |
| 5   | Contact kinggath about SS2 now or later | later                             |
| 6   | Git remote / publishing                 | local only                        |
| M3.O6 | UI (ADR 0007)                         | **decided 2026-10-14**: accepted with scope (read-only workbench, M4 lane W; authoring UI M5) |
| M4.O3 | SFSE / runtime extensions             | **decided 2026-10-14**: supported where technically justified (ADR 0008) |
| M4.O5 | Installing into MO2                   | **decided 2026-10-14**: `package` never; `kit deploy --dev` only (ADR 0009) |

### M2 owner tasks (lane O, `plan/execution-plan.md`)

| #     | Task                                                        | State                                  |
| ----- | ----------------------------------------------------------- | -------------------------------------- |
| M2.O1 | Approve ADR 0004, ADR 0005 Amendment 2, `docs/xedit-reference.md` | **done 2026-10-14** (with six clarifications) |
| M2.O2 | Whether OMOD is in scope                                    | **done 2026-10-14**: in scope (M2.B7)  |
| M2.O3 | Install SF1Edit (agents do not download executables)        | **done 2026-10-14 as M3.O3**: xEdit 4.1.5f at `C:\Modding\tools\xEdit\` (older than the pin; `docs/oracles.md`) |
| M2.O4 | CK and MO2 observation sessions                             | open; blocks M2.D1 check, M2.D3 (c), the `hardcoded-official-slots` check, M2.E3 `ck-diff` part |
| M2.O5 | Third-party mod plugins as inventory evidence               | open; default **no**                   |
| M2.O6 | In-game check of the rename plugin                          | **done 2026-10-14: PASS** (M2 fully closed) |

#### M2.O6: in-game check of the rename plugin (owner)

The plugin is `C:\Modding\kitkraft\out\OldSovietMarksmanRifle.esm` (2,221 bytes,
SHA-256 `9C0B28A6...4078EA27`, regenerated 2026-10-14 after lane E round 2 added `CNAM`
and `INCC` to its `TES4`; full hash and details in `docs/smoke-rename.md`). It
renames `Starfield.esm:0021BBCD` (WEAP `RussianHuntingRifle`, "Old Earth Hunting Rifle")
to "Old Soviet Marksman Rifle". In the Default profile the rifle's winner is
`Starfield.esm` (no mod overrides it, checked 2026-10-14 with `kit inspect --profile`), so
the patch's only master is `Starfield.esm`. Use a disposable save.

1. MO2: right-click the mod list > *Create empty mod* > `Old Soviet Marksman Rifle
   (kitkraft smoke test)`.
2. Copy the plugin into `C:\Modding\MO2-Starfield\mods\Old Soviet Marksman Rifle
   (kitkraft smoke test)\`.
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

Result: **PASS 2026-10-14** (recorded under "Done", "M2.O6 result").

### Divergence from the brief (for the owner)

The agents' docs diverge from `PLAN.md` in two places, now reconciled in `PLAN.md` ("AGENTS.md
wins") but still the owner's to confirm:

- **xEdit stance.** Resolved 2026-10-14: the owner decided ADR 0004 (pinned
  `wbDefinitionsSF1.pas` as the record-definition reference); `PLAN.md` and AGENTS.md agree.
- **No push.** The brief said "push to main"; AGENTS.md says commit to local `main`, no
  remote (owner decision 6).

## Requests (cross-lane)

- **Lane D (M4.D1, M4.D3) -> lane W.** `refs` and `planet` are in the catalog (`writes:
  false`, read only); `planet` needs the same treatment (a Planet view over `data.nodes`
  with `depth` indentation, or `data.placements`). `refs` in detail: it is in the catalog (`writes: false`, read only). The
  workbench has no generic envelope view a new command can use without code, so lane D
  only listed `refs` and `planet` in the gate test's `UNUSED_READ_ONLY` (its classification test
  requires every catalog command be classified). Please: move `refs` to `gate::ALLOWLIST` and
  `cmdline::SESSION_COMMANDS`; give the Records panel's inspect view "Outgoing links" and
  "Use info" actions (`refs {form}` and `refs {form, incoming: true}`) that list
  `data.edges` (path, status, `to`/`from` as links that run `inspect`, `placed.cell`/
  `placed.world` for placed referrers) with `next_cursor` paging, and show
  `limitations`. Result shape: `docs/mcp.md`, "`refs`".
- **Lane D (M4.D1) -> lane E (FYI).** `kitkraft_doc::links::{RESERVED_MASTER,
  RESERVED_OBJECT_IDS, NONE_FORM_ID}` restate validate's rule (a test in
  `handlers/refs.rs` keeps them equal); `validate` could use them, so the rule lives once.

- ~~**To lane F (from the F7 check):** `coverage`, `diff` and `roundtrip` reject a bare plugin filename and need the full Data path, while `load`, `inspect` and `query` fall back to KITKRAFT_DATA_DIR. Make bare names resolve the same way in every command (M4.F1 or earlier).~~
  Done in M4.F1 (Done, "Lane F, M4"): one resolver (`handlers/resolve.rs`) for every
  input name; `not_found` names the places searched.

Agents append here when they need a change in a crate they do not own.

- ~~**Lane E (M3.E9) -> lane B (M3.B9).**~~ Done (lane B round 4; used in overhaul build 2,
  `docs/overhaul/build-2.md`). The overhaul's open parts were OMOD property
  values: `ObjectModProperty.Value1` is opaque because xEdit decides it by the sibling
  `ValueType` (`wbDefinitionsSF1.pas@9fb0168...#L1981-L2031`, list `#L6894-L6938`), and
  the format only has `edid-prefix` unions, so this is format work (loader, codegen,
  `kitkraft_doc::typed`, `edit::fields`), not a TOML-only definition. Narrowest useful
  step: Value1 as `i32` when `ValueType` = Int (0) and `f32` when Float (1), every other
  variant raw (FormIDs stay opaque as now); corpus re-encode identical. It closes, with
  rules only: `WSPS` on receivers `077F13` (7.5 -> 5.625), `057555` (1.0 -> 1.5625),
  `04A750` (16 -> 12); `ACPT` on magazines `12D03F` (+5 -> +2), `087D4B` (+30, owner to
  confirm), `132C4A`/`132C49`/`132C48` (+8/+4/+16 -> +4/+2/+8). Details:
  `docs/overhaul/build-1.md`, "Why partial or open".
- ~~**Lane E (M3.E9) -> owner.**~~ Answered 2026-10-15 (B8 dropped; B11 base 18; B12 as
  built; `docs/overhaul/build-2.md`). Brief row B8 has no counterpart in the records (no
  automatic receiver OMOD carries a damage property); please restate it
  (`docs/overhaul/build-1.md`, B8). B11 and B12 also need a reading: which magazine
  "18 rounds" means for the Drum Beat, and whether B12's "14" is the Large (already 14
  after this build) or the Extra Large magazine.

- ~~**Plan maintainer -> lane B (round 5): implement ADR 0005 Amendment 5.**~~ Done (lane B
  round 5): `Confidence::ContainerReference` (`container-reference`, after `reference` in
  `Confidence::ALL`; `Confidence::below_corpus` ranks the two together), the loader rule
  (`uesp-skyrim:` citation, TES4/header files only), TES4 DATA/INTV set to it, parity's
  occurrence check both ways with no name list, `reference_members`, `build`'s gate, the
  `schema` limitations and the UI's unconfirmed marker; coverage and `kit schema` counts
  gain a `container-reference` bucket (a column in `docs/record-parity.md`). Was: new confidence
  `container-reference`. It needs a `uesp-skyrim:` citation and is accepted only in the
  container definitions (`TES4`, group header, record header). Steps:
  - Set it on `TES4.Master.FileSize` (`DATA`) and `TES4.TagifyCount` (`INTV`).
  - Drop `UNOBSERVED_ALLOWED` from `xtask/src/parity.rs`. The occurrence check skips
    `container-reference` as it skips `reference`, and fails when a `container-reference`
    member occurs in the corpus.
  - Rank it with `reference` in the engine's coverage counts, in
    `kitkraft_doc::confidence::reference_members` and in `build`'s `allow_reference` gate.
  - Update `crates/schema/defs/README.md`, rerun `cargo xtask codegen` and regenerate
    `docs/record-parity.md`.
  - Close the `docs/gaps.md` TES4 DATA/INTV row in the same commit.
- ~~**Plan maintainer -> lane E (overhaul build 2, M3.E9 rerun).**~~ Done 2026-10-15
  (`docs/overhaul/build-2.md`; owner row 3 now waits on the in-game check). Lane B round 4 typed
  `ObjectModProperty.Value1` (paths `DATA.Properties[n].Value1.Float` etc.). Rebuild
  `docs/overhaul/overhaul.toml` with the `WSPS`/`ACPT` rules named in "Lane E (M3.E9) ->
  lane B (M3.B9)" above, and fold in the owner's B8/B11/B12 answers when they come. Then hand the
  owner build 2 for M3.O5 (STATUS "Owner next actions", row 3).
- **Owner -> lanes H and B (M3.O3 done 2026-10-14).** SF1Edit is installed:
  `C:\Modding\tools\xEdit\xSFEdit64.exe` (file version 4.1.5.0). Lane H: run M3.H4's
  SF1Edit part with `KITKRAFT_SF1EDIT` pointing there, and update the SF1EDIT row of
  `docs/source-registry.md` ("not installed yet"). Lane B: M3.B11 is unblocked on the
  SF1Edit side; record the build's skew from the pin.
- **Policy -> lane D (later, for M4.E11, ADR 0009).** A read-only MO2 instance check in
  `crates/io` (`ModOrganizer.ini`, `mods\`, `profiles\`; reparse points reported) for
  `kit deploy --dev`.
- **Policy -> lane F (later, M4.W0, ADR 0007 Amendment 1).** `data` shape tests for the
  inspection commands and a catalog marking of writing commands, as the gate for lane W.
- **Policy -> all lanes (naming).** Write KitKraft in prose and use only the `kitkraft`
  identifiers: the rename is done (M4.BX1 integration, "Done" and Decision records).
- **Lane E (M4.E11) -> lane D.** `deploy` resolves the MO2 instance itself
  (`crates/engine/src/deploy/instance.rs`: `ModOrganizer.ini` `mod_directory`,
  `profiles_directory`, `overwrite_directory`, `download_directory`, `base_directory`,
  `selected_profile`; links and junctions refused) with `kitkraft_io::ini::Ini` and
  `Profile`. ADR 0009 wants a read-only instance type in `crates/io`; when it exists,
  `Instance::resolve` should call it (the settings logic duplicates part of
  `Profile::read_with`).
- **Lane E (M4.E11) -> plan maintainer / integrator.** ADR 0009 (unmerged branch) needs an
  amendment for the brief's layout: `.previous` folder in `mods\` instead of external
  backups (one kept), journal beside the folders, `dry_run` instead of `plan`,
  `kitkraft-deploy.json` (`docs/deploy.md`, "Where this differs"). `docs/mcp.md` could
  link `docs/deploy.md` under "Command results". When M4.E5 `package` lands, check its
  `kitkraft-manifest.json` against what `deploy` verifies (`files[]` of
  `{path, sha256, size}`, optional `plugins[]`).
- **Lane G -> lanes E, F, H (M4 lane G).** (1) Lane E: `package` (M4.E5's shape) landed in
  lane G (`handlers/package.rs`, `docs/packaging.md`), validating with `validate::run`
  and guarding `out` with `edit::patch::guard_location`; `testplan` (M4.E6) can read the
  folder's `kitkraft-manifest.json` for plugin hashes. (2) Lane F (M4.F1): `package`
  writes files (`destructiveHint`), `papyrus.events` is read-only; both are in the
  `docs/mcp.md` command sections. (3) Lane H (M4.H3): take event names for
  `docs/design/persistence.md` from `kit papyrus events --event <name>
  --declarations-only` (or `--script ObjectReference --kind event`); M4.G2's acceptance
  test that the cited events exist is added once that file is on main.
- **Integrator (M4.BX1) -> lane H (oracle comparator).** After the family merges the
  SF1Edit rerun shows 2,675 disagreements on the small files that are display conversions:
  enum and flag fields (number vs name or flag list), Wwise GUIDs (raw vs `{...}`), structs
  holding opaque or union fields, signature-valued fields. Converting those by kind (the
  schema has the names) would make the counts evidence (`docs/oracles.md`).
- ~~**Lane H -> lane D (strings), from the SF1Edit run (X8).**~~ Done (lane B round 5, at
  the director's request): `kitkraft_strings::Entry::text` shows an entry that is not
  valid UTF-8 as Windows-1252, one char per byte (the table `kitkraft-ba2` uses for
  archive names); bytes, `is_utf8` and `Resolved.bytes` are unchanged, and `strings`
  says the shown text is Windows-1252. Not rerun against SF1Edit. Was: some official string entries
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
  `SFBGS050`, `BlueprintShips-SFBGS050` and `SFBGS047` (KitKraft now assumes right after
  the `Starfield.ccc` plugins, `hardcoded-official-slots`): the runtime IDs of their objects
  in game would show their slots. Lead: the CK's own `EditorDataFilesLoaded.txt` in the game
  dir lists each loaded file with its index and ID form (`001 - (FE000XXX) -
  'KZ_Mantis.esm'`); a CK session with the full profile would record the CK's order.
- **Lane D -> lane H / owner (M3.O2 script, from M3.D6).** BlueprintShips-SFBGS050.esm is
  localized but ships no string tables; its 66 LCTN and 30 GBFM names (FULL) resolve
  nowhere in KitKraft. In game, a location from it (for example the first unresolved LCTN
  `kit inspect` lists) would show whether the game reads its names from SFBGS050's tables
  (30 of the 96 IDs exist there) or shows none. **Lane H:** in the M3.O2 script
  (`docs/ck-observations.md`, step 3) with `BlueprintInteriorSFTER_TerranShip_Generic_TESTLocation`
  (`FULL` 0x0002E9C4; none of the 66 LCTN name IDs is in `sfbgs050_en.strings`).
- **Lane D -> lanes E, F (review round 2 API changes).** A deleted version is
  `RecordLoc.flags & kitkraft_doc::flags::RECORD_FLAG_DELETED != 0` (`inspect` and `query`
  show `deleted`), for lane E's refusal of deleted sources. `Version` has `basis`
  (`kitkraft_doc::Basis`: `Direct`, `FullStyleSelf`, `FullStyleSelfMasked`);
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
  (`kitkraft_doc::SourceCheck { winner, blocking, unrelated }`, `is_safe()`): refuse on
  `blocking`, report `unrelated` as limitations (M3.E1). (2) `Resolved::bytes` and
  `Resolved::utf8` (M3.E2).
- **Lane E -> owner (M2.O4).** The `ck-diff` half of M2.E3 is open: what the CK writes for
  a new non-localized plugin (master flag for `.esm`, `HEDR` next object ID, inline text
  encoding: we write UTF-8). Until observed it is a limitation on every `set`/`build`.
- **Lane F -> lanes D, E, G (session and catalog closure).** A handler that needs the
  load order is a `fn(&mut Session, &Value) -> Result<Outcome, EngineError>` added to
  `handlers::session_defaults()` (stateless handlers stay in `defaults()`). Keep your
  typed state in a slot (`Session::insert` / `get` / `get_mut`); `load` also sets
  `Session::set_load_order(Some(listing))`, which `kitkraft://load-order` serves as is.
  When you flip a command to `Implemented`, removing its `PLANNED_IN_M2` line in
  `crates/engine/src/lib.rs` is optional (a stale entry only prints); a command you will
  not finish in M2 becomes `Status::Deferred { reason, milestone: "M3" }`.
- **Lane G -> lane D (M2.G8 landed; for M2.D7).** `kitkraft-strings` API:
  - `Kind::{Strings, DlStrings, IlStrings}`, `Kind::from_path(&Path)`,
    `Kind::from_extension(&str)`, `kind.extension()`;
    `table_name("Starfield.esm", "en", Kind::Strings) == "starfield_en.strings"`.
  - `Table::parse(kind, &bytes) -> Result<Table, Error>`, `Table::read_file(&Path)`;
    `table.get(id: u32) -> Option<&Entry>`; `Entry { id, bytes, offset: Option<u64> }`
    with `entry.text() -> Cow<str>` (lossy UTF-8), `entry.is_utf8()`,
    `entry.stored_len(kind)`; `offset` is the absolute offset in the table file (evidence).
    `Error` variants carry byte offsets.
  - Tables in archives: `kitkraft_ba2::Archive::open(&path) -> (Archive, File)`,
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
  entry above it is `kitkraft_ba2::Error::TooLarge`, reported like any other table error.
- **Lane G -> lane E (for M2.E8).** `kitkraft_pex::Pex::parse(&bytes)`; calls are
  `Instruction`s with `opcode_name(op)` `callmethod` / `callstatic` / `callparent`, the
  function name in `args[0]` (`Value::Identifier`, resolve with `pex.string(i)`), the base
  object in `args[1]`; `.pex` inside archives via `kitkraft_ba2` as above, or
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
- **Lane A -> lane D (M2.A1, M2.A2 landed).** `kitkraft_esm::index`:
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
  `kitkraft_esm::build`: `Plugin::create(&HeaderSpec)`; `HeaderSpec::new(masters)` fills
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
- ~~**Lane F -> lane A (M3.F2).** `esm::bridge::extra_of` still matches five variants (GLOB,
  GMST, KYWD, TES4, WEAP), so `check_subrecords` misses `NotDecoded` conflicts in AMDL,
  AMMO, COBJ, OMOD and ZOOM (and in every round 3 type); `typed.extra()` covers them all.~~
  Done in lane B's M3.B1 commit (FYI to lane A): `extra_of` returns `typed.extra()` for
  every record type, a one-function change, so `corpus_full_roundtrip`'s typed check sees
  undecoded payloads in every round 3 type. Lane A (M4) then removed `extra_of`:
  `check_subrecords` reads the generated `Record::extra()` directly, and a test checks every
  defined type (merged in M4.BX1).
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
  After merging main (lane B's merge commit), three more expectations followed the typed
  schema: `handlers::coverage` (the M3.F2 `failed` counts with MODL typed; 8 decoded by
  confidence), `handlers::overhaul_tests` (KSIZ is looked up by name: WEAP's numeric
  members now come first) and `tests/diff_corpus.rs` (picks a record type whose definition
  places KWDA, since lstring-only definitions keep it raw).
- **Lane A -> lane E (M4.A1..A6 landed; for M4.E1, E2, E3, E4).** `kitkraft_esm`:
  - `forms::NewForms::for_index(&index, &master_scales)` / `for_plugin(&plugin,
    &master_scales)` / `new(&tes4, &master_scales, own_max)` -> `allocate() ->
    Result<u32>`, `write_hedr(&mut tes4)`; `master_scales: &[Option<tes4::Scale>]` is
    each master's scale in master-list order (`Scale::of_flags(master_tes4.flags())`, or
    from lane D's load order); the own prefix is the **full**-master count, and an
    unknown (`None`) scale, a wrong-length list or a small/medium target is
    `Error::Refused` (fix round, review blocker: masters [Starfield full, Constellation
    small, SFBGS003 medium] give 01xxxxxx). One allocator per plugin per edit session
    (not `Copy`). Name `forms.limitations()` in results (`LIMITATIONS` plus a low `HEDR`
    next object ID and the blueprint flag's `HEDR` count meaning).
    `forms::names_master(form_id, &master_scales) -> Option<bool>` classifies a stored
    FormID.
  - `refs::compose_reference(&RefSpec, allow_reference: &[&str]) -> Result<ComposedRef>`
    (`RefSpec::new(sig, form_id, base, position, rotation)`, then `persistent`,
    `initially_disabled`, `editor_id`, `extra`...); `ComposedRef { record, position,
    members, limitations }`; `composed.placed(place, cell, &style) -> Result<Placed>`
    (`&mut self` since the fix round: a position outside its exterior cell adds a
    limitation; non-finite numbers, a NUL in the EditorID and `EDID`/`NAME`/`DATA`/`XXXX`
    in `leading`/`extra`/`trailing` are refused). Until BW's
    `REFR`/`ACHR` definitions reach `main` through BX, `DATA.Position.Z` and
    `DATA.Rotation` are below `corpus` and must be in `allow_reference` (nothing to change
    when they land). They have landed (M4.BX1): both come from the REFR/ACHR `DATA`
    definition at `corpus`.
  - `cells::cell_place_in(&index, &loc, &cell_record, &limits) -> Result<CellPlace>`
    (interior / world persistent cell / exterior with its `XCLC` grid),
    `cells::persistent_cell(&index, world) -> Option<&RecordLoc>` (where a persistent
    reference in a world goes), `cells::Grid::of_position(x, y)` (cell of a position,
    100 units), `cells::placement(&header, &Parent, &style)` for any record under a cell,
    quest or topic; `Plugin::upsert_styled` / `RewriteOptions::placement` place in corpus
    order. A placed reference needs its parent `CELL` (and `WRLD`) carried in the plugin
    first: since the M4 placement-core fix round, `upsert_styled` and the streaming upsert
    refuse (`Error::Placement`, "carry the WRLD/CELL override first") a children group
    (types 1, 6, 7, 10) whose record is not in its parent group; `cells::carry_parents(
    &master_index, &place, cell, &style, &limits, read)` gives the lossless `WRLD`/`CELL`
    overrides to upsert first (skip those the target has). Before the fix the group was
    created without its record, a shape no official file has. **Lane E:** the engine's
    `edit::source::PARENT_LABELLED` is `[1, 6, 7]`; a copied `DIAL`/`SCEN`/`DLBR` (or an
    `INFO`, whose `DIAL` sits in type 10) also needs its `QUST`, so add 10 there, or such
    a patch is now refused instead of getting an ownerless quest children group.
  - `navmesh::import_navmeshes(&ck_plugin, &mut target, &NavmeshOptions { style, limits,
    master_scales })` and `navmesh::navmesh_placements(&ck_plugin, &target_index,
    read_cell, &options)` for `[[import]]`; refused unless both plugins have the same
    masters. `NavmeshImport::master_forms` lists imported NAVMs that override a master's
    (ONAM candidates), `unknown_forms` those a missing scale leaves undecided.
  - `CellPlace::Interior { labels }` (fix round): an existing interior cell keeps the
    block/sub-block it was read in (`cell_place_in`); `CellPlace::INTERIOR` computes them
    for a new cell. `PlacementStyle::detect(&index)` reads a target's style back.
    `Upsert::Moved`: a reference whose flag 0x400 changed leaves its cell's other
    children group (engine's patch report says "moved"). Paths are checked against flag
    0x400 and a second persistent `CELL` per world is refused.
  - ONAM (M4.A6): when a patch overrides records in groups 7, 9 or 10, set
    `Tes4::set_overridden_forms(&mut tes4, &cells::onam_forms(&plugin, |id| <id is a
    master's and the master has it>))` (`cells::onam_forms_in(&index, pick)` for a
    streamed patch); patches that only override top-level records or persistent
    references still need none. INCC stays yours (as M2.E3).
- **Lane E -> lanes D, F (M3 round 1).** (1) Lane F: new catalog command
  `overhaul.scaffold` (alias `overhaul`, session handler registered in
  `handlers::session_defaults`), `build` gained `plan`, `validate` gained
  `plugin_archives` and `.ba2` inputs in `scripts`; `docs/mcp.md` has their sections, the
  command list (M3.F3) may want them. **Lane F:** (1) answered: the sections are the
  command list; no shortcut tool (`execute` and `command_line` reach it, and the tool
  descriptions and `kitkraft://commands` list it from the catalog). (2) Lane D: `build`'s `via` follows typed FormID
  members through `VersionSubject::lookup` locally (`handlers/build.rs` `follow`); M3.D4's
  `follow(form, member path)` should replace it when it lands. M3.E1/E2 wait for M3.D1.
  **Done (lane D, M4.D1):** `build` calls `kitkraft_doc::links::follow`; the local one is
  gone.
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
- **World schema lane -> lane A (child-group placement).** The world family is typed
  (`docs/schema/world-family.md`). To add cells and placed references, the builder needs
  to derive the group path itself: interior CELL -> top `CELL` group, type 2 block and
  type 3 subblock (labels from the FormID's object part, mod 10 and div 10 mod 10: 17,019
  of 17,020 official cells; the exception is a small-master FormID), then the CELL, its
  type 6 children right after it, then type 8 (persistent) or 9 (temporary); exterior
  CELL -> the world's type 1 group, type 4 block (Y div 32, X div 32 of `XCLC`) and type 5
  subblock (div 8), both labels two i16 with Y first (22,682 of 22,682), then as above;
  NAVM goes in type 9. Type 8 and 9 occur in either order inside type 6 (most cells have
  type 9 only). The full corpus counts and the asserted rules are in the doc and in
  `crates/esm/tests/world_family.rs` (`world_family_full`).
- **World schema lane -> lane B (schema owner).** CELL, REFR and ACHR need an `unordered`
  record mode (match members in any order, keep the input order for encoding): with the
  ordered matcher 889,512 REFR, 6,434 ACHR and 502 CELL records keep some subrecords in
  `extra` (counts per signature in the member notes). Until then the definitions use the
  best linear order. Also, the world family adds shared types `PositionRotation`,
  `OwnerData`, `EnableParent`, `ColorRgb`, `ColorRgba`, `PropertyValue`, `RagdollBone` and
  groups `LinkedReferences`, `GroupedPackIn`; ~~if round 3 adds equivalents (`Vec3`, `Bool8`
  exist in the round 3 worktree), fold them together at integration.~~ Done in M4.BX1 (WORLD
  merge): PositionRotation's position and rotation and ObjectBounds' corners are `Vec3`,
  OwnerData's NoCrime is `Bool8`. The unordered mode is done too (M4.BX1, integrator,
  2026-10-14; see Done).
- **World schema lane -> lane D.** `kitkraft_doc::typed::field_type` picks a layout by
  signature, which is ambiguous for REFR `INAM` (patrol idle FormID vs pack-in u16) and
  wrong for subrecords inside BFCB bodies that share a member's signature (WRLD `PNAM`,
  `NAM2`..`NAM9`). `crates/doc/tests/typed_corpus.rs` now maps each subrecord to the member
  the generated matcher placed it in (`match_members` hits); `inspect`/`where`/`diff`
  should do the same. Done in M4.BX1 (positional layouts, `typed::placements`/`layout_at`);
  `typed_corpus.rs` uses them.
- **World schema lane -> lane A (bridge).** `esm::bridge::extra_of` still lists five
  variants; the seven world types also need `typed.extra()` for `check_subrecords` to report
  `NotDecoded` (lane F's request above). `world_family.rs` uses `Record::extra()` directly.
  Done: `extra_of` returns `typed.extra()` for every type (lane B round 3, merged in M4.BX1).
- **Integrator (M4.BX1) -> lane E.** `formid.dangling` flags object ID 0x14 in reference
  slots whose targets include PLYR (13 errors in Starfield.esm, now `KNOWN_ERRORS`); if a
  source confirms 0x14 is the engine's player reference, resolve it there. **-> owner/lane H
  (CK check):** REFR `XPLK` references in SFBGS00D, SFBGS050 and ShatteredSpace are stored
  one file index above the plugin's own (38 occurrences); does the CK write or read them
  that way?
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
- ~~**Lane A -> lane F (M3.A2 landed; for M3.F1).**~~ Done in M3.F1 (Done, "Lane F,
  M4"): `roundtrip --decode`, exit 1 end to end. `RoundtripOptions::decode:
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
- ~~**Planet schema lane -> lane D.** `kitkraft_doc::typed::field_type` looks a layout up by
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
  `kitkraft_doc::typed`, see Done); the GBFM object-template FULL can be typed again
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
- **Quest schema lane -> BX (integrator).** Done in M4.BX1 (QUEST merge, see Done): (1) Regenerate the four derived files on merge
  (see Done, quest family). (2) `common/Condition.toml` (PLANET's, which a family lane may
  not edit) keeps CIS1/CIS2 at `reference`, but they occur in this family (CIS1: QUST 14,
  PACK 2, SCEN 1; CIS2: QUST 43, PACK 18, INFO 115, SCEN 34) and decode identically: raise
  them to `corpus` with that evidence, and add the family's 102,280 CTDA (all decode)
  to the Condition evidence. (3) `validate_corpus_official_files` (ignored, release) with
  this family regenerated reports 22 new errors in Starfield.esm, all object IDs below
  0x800 that no official file defines, the class of the PKIN `KNOWN_ERRORS`: SMEN `PNAM`
  = 0x5B on 20 event nodes and QUST `QuestReferenceAlias.ALFR` = 0x14 on 2 aliases (plus
  warnings: SMEN in SFBGS003 and ShatteredSpace, QUST in SFBGS00D and ShatteredSpace, 1
  each). Please list them as known with that evidence. INFO TRDA Emotion was demoted to
  raw in this lane (0xFFFFFFFF sentinel, 53,844 lines). (4) No duplicate shared blocks
  expected from this lane; BC/BI may
  define their own script-attachment block: this lane's is a `type` (VMAD payload), not a
  group.
- ~~**Quest schema lane -> lane B (codegen library, `crates/schema/src/codegen.rs`).**~~
  Done (lane B round 5): the generator no longer mimics rustfmt's widths (one-line
  calls, arms and struct literals, no single-literal special case); `cargo xtask codegen`
  formats every file it writes and the golden tests format `generate()` the same way
  before comparing, so long names and one-member shared groups cannot break the gate.
  Regenerated output and goldens were byte-identical. Was: the
  generator's rustfmt mimicry has two gaps the golden test (`golden_files_are_rustfmt_clean`)
  hits: (1) a shared group with exactly one member emits a one-element `MEMBERS` array
  rustfmt would join onto one line; (2) a call whose arguments exceed rustfmt's call width
  (long member paths in `read`, long field names in `emit`) is not broken. This lane
  worked around both (ScriptAttachment is a type; shorter member names), but other lanes
  will meet them.
- **Quest schema lane -> lane A.** As the planet lane's entry: `bridge::extra_of` lists five
  types, so `check_subrecords` reports no `NotDecoded` for the ten quest family types; the
  out-of-tree check covered it.
- **Quest schema lane -> lane H (CK observation).** Questions only the CK can answer (details
  in `docs/schema/quest-family.md`, "Open questions"): (1) QUST alias ALFG: float or
  integer (official values 0 and the bit pattern 2)? (2) QUST create-matching ALCA: 4 bytes
  in the corpus, 6 upstream; what they hold. (3) QUST collection ALAM: which CK field (0,
  twice 0x40000000). (4) INFO ENAM: which flag set the CK shows for an info group. (5) SCEN
  VNAM (20 bytes, every scene) and INFO INTV (4 bytes after TSCE): which CK fields.
  (6) SMEN PNAM = 0x5B on every event node and QUST ALFR = 0x14 on two aliases: what
  those low object IDs are to the CK (engine-reserved forms?). (7) Whether the CK
  writes a CITC before quest, scene, package-level and response conditions on a new record
  (no official one has it).
- **Actor schema lane (BC) -> integrator (BX).** Done in M4.BX1 (ACTOR merge, see Done; (2) by the engine-reserved ID rule, (3) CIS1 was raised at the QUEST merge, (4) model and destructible folded, PRPS at the ITEMS merge). At the ACTOR merge: (1) add the 9 records
  and `common/{AttackData,Effect,SoundReference}.toml` to `BUILTIN_DEFINITIONS`, the
  `tests/defs.rs` lists and the golden; (2) `validate_corpus_official_files` then reports
  319 new `formid.dangling` errors in Starfield.esm, all explained, none a layout error:
  318 are `PRPS.ActorValue` values below 0x800 that no file defines, the same class as the
  known PKIN ones (NPC_ 139: 0x385 x102, 0x32D x17, 0x330 x7, 0x3A2 x7, 0x39E x5, 0x351
  x1; RACE 178: 0x2D8, 0x2D9, 0x2E0 x31 each, 0x39E, 0x39F, 0x3A0, 0x3A1 x21 each, 0x385
  x1), plus `NPC_:0018B5F4` `RDSA.SpecificReference = 00577823`, a stale value in an
  entry whose flags do not enable it; the DLC files report the same actor values as
  warnings. Please list them in `KNOWN_ERRORS`. (3) `common/Condition.toml`'s CIS1 is
  `reference`, but one official PERK condition carries a 40-byte CIS1 that decodes; it can
  be raised to `corpus`. (4) Duplicate candidates: the inline PRPS property layout (NPC_,
  RACE, CLAS, as in PKIN), the model group (LVLN, as in LVLP) and the destructible block
  (NPC_, kept opaque) may meet other families' blocks. (5) Codegen note: the generated
  code follows rustfmt only up to its call-width heuristic; long member paths make the
  golden `seed.rs` not rustfmt-clean (worked around with shorter NPC_/RACE names). (5) done in lane B round 5: codegen output and goldens are formatted by rustfmt, no width mimicry.
- **Actor schema lane (BC) -> lane H (CK observation).** Questions only the CK can answer
  (details in `docs/schema/actor-family.md`, "Open questions"): (1) actor values below
  0x800 in `PRPS` (NPC_, RACE, PKIN): does the CK name them and keep them on save? (2) does
  the CK write `STOP` on an NPC_ without an object template, and keep PRKZ, COCT, CS3H,
  EDCT, OBTE and LVLN LLCT in sync with their lists? (3) how `ACBS.Level` reads and is
  written with `PcLevelMult`; (4) which template applies to a section whose flag is set and
  whose `TPTA` slot is 0; (5) which controls set each `ONA2` override flag and the block
  order; (6) whether `RDSA.SpecificReference` is cleared when `UseSpecificReference` is
  off; (7) EPFT against the EPFD sizes (4, 8) and whether the CK writes a 3-byte PERK
  `DATA`; (8) any CK field that changes NPC_ `NAM5` (always `FF 00`), RACE `DAT2` offsets
  92-127 or the chargen NNAM.
- **Items/workshop schema lane (M4.BI1) -> BX (integrator).** Done in M4.BX1 (ITEMS merge, see Done; (2) by the engine-reserved ID rule of ADR 0005 Amendment 4). (1) Derived files to
  regenerate at merge: `crates/esm/src/generated/` (`cargo xtask codegen`), the
  `BUILTIN_DEFINITIONS` region (12 records, 11 `common/` files), `crates/schema/tests/
  golden/seed.rs` (`KITKRAFT_UPDATE_GOLDEN=1`) and the record and common lists in
  `crates/schema/tests/defs.rs`. (2) `validate_corpus_official_files` (`#[ignore]`) then
  reports 646 new `formid.dangling` errors in Starfield.esm, all `ObjectProperty.ActorValue`
  (the PRPS element) naming object IDs below 0x800 that no official file defines, the same
  fact as PKIN's known 0x33D/0x33E: ACTI 128, CONT 443, FURN 13, STAT 61, TERM 1; by value
  0x330 207, 0x3A2 230, 0x33E 70, 0x2D0 62, 0x33D 31, 0x32E 19, 0x32F 19, 0x387 6, 0x338 1,
  0x3AC 1 (the DLC files report the same values as warnings). Extend `KNOWN_ERRORS` or,
  better, decide once how actor values below 0x800 are treated (the open CK question from
  the planet lane). (3) Shared blocks to fold: `common/Model` vs LVLP's inline `Model`
  group and the weapon family's inline MODL/FLLD members; `common/ObjectProperty` vs PKIN's
  inline PRPS element; `common/ObjectTemplate` vs GBFM's inline OBTE/Combination and WEAP's
  `ObteBlock`; `common/SoundReference` vs any inline sound slot another family types
  (WEAP, ASPC, KSSM use the same upstream helper); `ColorRgb` is RGB plus a pad byte, not a
  duplicate of `ColorBgra`. (4) Ignored release corpus tests with these definitions: esm
  `corpus_full_roundtrip` passes (all 14 identical; M2.A3 typed check 0 conflicts, 0
  unchecked), doc `typed_corpus` passes (80,652 typed records, the same 181 pinned
  unresolved lstrings); engine `validate_corpus_official_files` fails only on (2).
- **Items/workshop schema lane -> lane E (FYI, done in its own commit).** Two unit tests
  used ACTI ATTX as their example of an lstring the schema does not type
  (`edit::fields::tests::layouts_and_fallback`,
  `patch_tests::untyped_lstrings_of_localized_sources`); ACTI is typed now (ATTX as an
  lstring), so both examples moved to FLOR ATTX (no definition, an lstring in the corpus
  table). Test-only change.
- **Items/workshop schema lane -> lane E and the plan (W3 row).** `WKMF` is "Wwise Keyword
  Mapping" (sound selection for ship engines, cockpits, grav drives, creature animation),
  not a workshop record; W3 does not need it. What registers an outpost object is the COBJ
  BNAM category keyword (`docs/schema/items-workshop-family.md`, input for M4.E7).
- **Items/workshop schema lane -> lane H (CK observation).** Questions only the CK or the
  game can answer (details in `docs/schema/items-workshop-family.md`, "Open questions"):
  (1) which COBJ field the CK writes as BNAM and as FNAM for an outpost recipe, and which
  the build menu reads; (2) whether a PTT2 with a workshop-icon transform is required for
  an object to show in the outpost menu; (3) which CK checkbox sets record flag 0x10000000
  on outpost furniture; (4) the CK fields behind FURN FNAM 0x1, CONT DATA 0x8, MNAM bit 29,
  CPTH DATA 0x8, IDLE FNAM 0x10 and TERM's one-byte WBDT; (5) the CK names of TMLM INAM,
  TMLM XLOC's last u32 and STAT DNAM's second float; (6) whether the CK writes ARMO's
  FLLD/XFLG inside the world models; (7) what references the `OutpostFoundation*List`
  form lists; (8) for M4.O4 on `KitKraft Dev`: does a new recipe with a vanilla
  `OutpostCategory*` BNAM show at every beacon, and does a new category keyword need the
  beacon and `OutpostRecipeCategories`.
