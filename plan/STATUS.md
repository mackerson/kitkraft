# Status

Updated: 2026-10-14

## Current milestone: M2 (agent surface)

M0 (scaffold) and M1 (data parity) are done. Every "M1 is done when" criterion in
`plan/execution-plan.md` holds: M1.6 roundtrips all 14 corpus files in `--release` and M1.7
passes (lane A); `kit roundtrip --verify` reports identical on all 14 (M1.13, rechecked by
`cargo xtask parity`); `docs/record-parity.md` is generated (M1.14); `cargo xtask ci` is
green. M2 is not broken down into tasks in `plan/execution-plan.md` yet; the lanes are under
"Next".

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

## In progress

- Nothing.

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
| schema     | Typed layouts from ck-diff observations or `uesp-starfield` pages confirmed on the corpus                      | `crates/schema` (lane B)                     |

## Blockers

- None.

## Open owner decisions

| #   | Decision                                | Default until decided             |
| --- | --------------------------------------- | --------------------------------- |
| 1   | Project name                            | `KitCraft` / `kitcraft-*` / `kit` |
| 2   | xEdit MPL source policy (ADR 0004)      | option A: ADR 0005 sources only   |
| 3   | Video critique used as overhaul brief   | none; overhaul work is after M2   |
| 4   | License                                 | MIT OR Apache-2.0 (scaffolded)    |
| 5   | Contact kinggath about SS2 now or later | later                             |
| 6   | Git remote / publishing                 | local only                        |

### Divergence from the brief (for the owner)

The agents' docs diverge from `PLAN.md` in two places, now reconciled in `PLAN.md` ("AGENTS.md
wins") but still the owner's to confirm:

- **xEdit stance.** The brief called xEdit's MPL source "readable as a reference"; AGENTS.md
  and ADR 0004 keep it off limits (option A) until the owner decides. ADR 0004 has been
  escalated: UESP's Starfield pages are thin (ADR 0005 Amendment 1), so under option A
  most Starfield field layouts come from CK differential observation, which is slower. The
  owner has been asked.
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
- **Lane B -> owner (default chosen).** Inventory definitions name members after their
  signature (`Edid`, `Wfir`, `Flld2`); renaming them once meanings are sourced (`ck-wiki`)
  is a breaking change for `kit query`. Default: accept the rename when it happens.
- **Lane C -> lane B.** `cargo xtask codegen` can own `BUILTIN_DEFINITIONS` once
  `crates/schema/src/lib.rs` marks a generated region for it (for example
  `// BEGIN generated by cargo xtask codegen` / `// END`); until then
  `tests/defs.rs` is the guard and xtask leaves the file alone.
