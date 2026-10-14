# Status

Updated: 2026-10-14

## Current milestone: M1 (data parity)

M0 (scaffold) is done. M1 lane A is done; lane C has M1.14/M1.15 left (blocked on lane B
round 2); see `plan/execution-plan.md`.

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
- Source policy (owner decision 2026-10-14): ADR 0004 **decided**: upstream xEdit
  `Core/wbDefinitionsSF1.pas` pinned at `9fb016884bec138ea6c7b872cec831537d464c3e` is the
  record-definition reference, source material only. `docs/xedit-reference.md` records the
  verification (default branch `dev-4.1.6`, why no tag is pinned, MPL-2.0, all 18 lane B
  record types defined, line spans, Pascal-to-TOML map). ADR 0005 Amendment 2 (four-level
  hierarchy, SF1Edit differential comparison, Skyrim container-only), ADR 0001 Amendment 1
  (clean room), AGENTS.md, PLAN.md, README, NOTICE, ATTRIBUTION updated. Schema loader:
  `xedit:` sources (pinned SHA + line range) and `confidence`
  (`reference`/`corpus`/`ck-diff`/`verified`; required for `xedit:` and `uesp-skyrim:`
  citations); TES4, GRUP, RECORD carry `confidence = "corpus"`. `*.pas` gitignored.

## In progress

- **Lane B is paused** until the owner reviews `docs/xedit-reference.md` and ADRs 0004,
  0005 (Amendment 2) and 0001 (Amendment 1).

## Next

1. M1 lane A: done for M1. Next per the plan is M2 groundwork, or `roundtrip --decode`
   (decompress every compressed record during the gate; see `docs/gaps.md`).
2. M1 lane B: round 3, after the owner review above: transcribe typed definitions from the
   pinned xEdit file (`xedit:` provenance, `confidence = "reference"` until the corpus
   confirms), in this priority: **WEAP, AMMO, PROJ, AMDL, OMOD, COBJ, ZOOM**; then
   **GMST, GLOB, KYWD** typed; then **LVLI, FLST, INNR**. Each definition is checked against
   every corpus occurrence before it moves to `confidence = "corpus"`; unions stay opaque.
3. M1 lane C: M1.14 `coverage` + `cargo xtask parity` and M1.15 `cargo xtask codegen
   [--check]`, unblocked by lane B round 2 (see Requests).

## Blockers

- None.

## Open owner decisions

| #   | Decision                                | Default until decided             |
| --- | --------------------------------------- | --------------------------------- |
| 1   | Project name                            | `KitCraft` / `kitcraft-*` / `kit` |
| 2   | xEdit MPL source policy (ADR 0004)      | **decided 2026-10-14** (pinned xEdit SF1 definitions as reference) |
| 2a  | Review `docs/xedit-reference.md` and ADRs 0004/0005/0001 before lane B round 3 | lane B paused |
| 2b  | Whether `wbDefinitionsCommon.pas` (helpers the SF1 file uses) may be cited too | no: only `wbDefinitionsSF1.pas` |
| 3   | Video critique used as overhaul brief   | none; overhaul work is after M2   |
| 4   | License                                 | MIT OR Apache-2.0 (scaffolded)    |
| 5   | Contact kinggath about SS2 now or later | later                             |
| 6   | Git remote / publishing                 | local only                        |

### Divergence from the brief (for the owner)

The agents' docs diverge from `PLAN.md` in two places, now reconciled in `PLAN.md` ("AGENTS.md
wins") but still the owner's to confirm:

- **xEdit stance.** Resolved 2026-10-14: the owner decided ADR 0004 (pinned
  `wbDefinitionsSF1.pas` as the record-definition reference); `PLAN.md` and AGENTS.md agree.
- **No push.** The brief said "push to main"; AGENTS.md says commit to local `main`, no
  remote (owner decision 6).

## Requests (cross-lane)

Agents append here when they need a change in a crate they do not own.

- **Lane B -> lane C (M1.15, M1.14).** `cargo xtask codegen` should write
  `kitcraft_schema::generate(kitcraft_schema::builtin()?)` (`builtin()` now returns a cached
  `Result<&'static Schema, &'static SchemaError>`) to `crates/esm/src/generated/mod.rs`. The
  output is self-contained (std only), rustfmt-clean for the current set (checked by a
  test), and lint-clean outside `cfg(test)` (`crates/schema/examples/generated.rs`). Suggest
  still piping it through `rustfmt --edition 2021` before writing/comparing, since the
  generator only mirrors rustfmt's width rules for the constructs it emits today. xtask may
  also take over generating `BUILTIN_DEFINITIONS` (today `crates/schema/tests/defs.rs`
  fails with the lines to add when `defs/` and the list differ). For coverage:
  `RecordDef::leaf_subrecords()` (member paths), `Schema::known_signatures(sig)`,
  `Signature::from_bytes`, and the generated `RECORDS` table.
- **Lane B -> lane A.** Generated records decode from `&[RawSub { signature: [u8; 4], data:
  &[u8], oversized: bool }]` (subrecords after `XXXX` resolution; `oversized` = the size came
  from `XXXX`) plus `Ctx { form_version, localized }` (`localized` = TES4 flag `0x80`), via
  `Record::decode(signature, subs, ctx)`, and encode to `Vec<OwnedSub>` carrying the same
  flag; write `XXXX` when it is set or the payload exceeds `u16::MAX`. The 24-byte record
  header is now a definition too (`RECORD.toml`, generated `RecordHeader`). Also please
  count `XXXX` subrecords with a payload <= 0xFFFF in the corpus inventory (`docs/gaps.md`).
- ~~Lane B -> owner / AGENTS.md maintainer: point AGENTS.md at ADR 0005 Amendment 1.~~
  Done 2026-10-14: AGENTS.md "Clean room" now states the Amendment 2 hierarchy, with UESP's
  Starfield pages at level 4.
- **Policy -> lane B.** `confidence` is validated by the loader but not stored in the model
  (the policy change was limited to `load.rs`). Add it to `RecordDef`/`HeaderDef`/`CommonDef`
  so coverage can report it, and consider re-exporting the pinned revision for tools.
- **Policy -> lane C (M1.14).** `docs/record-parity.md` should show `confidence` next to the
  source kind once the model carries it; `confidence = "reference"` values should surface as
  a `limitations[]` entry in `inspect`/`query` results.
- **Policy -> M2.** Install SF1Edit (matching the pin if possible, else record the build) for
  the differential comparison in ADR 0005 Amendment 2 §2a; never bundle it.
- **Lane B -> owner (default chosen).** Inventory definitions name members after their
  signature (`Edid`, `Wfir`, `Flld2`); renaming them once meanings are sourced (`ck-wiki`)
  is a breaking change for `kit query`. Default: accept the rename when it happens.
