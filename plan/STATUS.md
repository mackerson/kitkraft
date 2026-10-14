# Status

Updated: 2026-10-14

## Current milestone: M1 (data parity)

M0 (scaffold) is done. M1 lanes A, B and C can start; see `plan/execution-plan.md`.

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

## In progress

- Nothing.

## Next

1. M1 lane A: M0.5 corpus test helper, then M1.1 bounded reader (M1.2b header inventory after M1.2).
2. M1 lane B: M1.8-M1.10 done. Next: typed WEAP layouts only via ck-diff observations
   (ADR 0005 section 2); more corpus inventories as M1.14 coverage asks for them.
3. M1 lane C: M1.11 `execute` and the result envelope.

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
  escalated: with no UESP Starfield page (ADR 0005), option A means Starfield field layouts
  come only from CK differential observation, which is slower. The owner has been asked.
- **No push.** The brief said "push to main"; AGENTS.md says commit to local `main`, no
  remote (owner decision 6).

## Requests (cross-lane)

Agents append here when they need a change in a crate they do not own.

- **Lane B -> lane C (M1.15).** `cargo xtask codegen` should write
  `kitcraft_schema::generate(&kitcraft_schema::builtin()?)` to
  `crates/esm/src/generated/mod.rs`. The output is self-contained (std only), rustfmt-clean
  for the current set (checked by a test), and lint-clean. Suggest still piping it through
  `rustfmt --edition 2021` before writing/comparing, since the generator only mirrors
  rustfmt's width rules for the constructs it emits today.
- **Lane B -> lane A.** Generated records decode from `&[RawSub { signature: [u8; 4], data:
  &[u8] }]` (subrecords after `XXXX` resolution) plus `Ctx { form_version }`, and encode to
  `Vec<OwnedSub>`. The container keeps emitting original bytes for unmodified records.
- **Lane B -> owner (ADR 0005 premise, decision for the owner; default: not used).** UESP's
  Starfield namespace does exist, hosted at `https://starfieldwiki.net/wiki/Starfield_Mod:Mod_File_Format`
  (served by uesp.net), with only two subpages: `/GMST` and `/Raw_Data` (a per-record
  subrecord count/size dump of an older Starfield.esm). There are no TES4, GLOB, KYWD or WEAP
  pages. Nothing from it is cited in `crates/schema/defs/`; every inventory fact there comes
  from our own corpus scan. Decide whether ADR 0005 should list it.
- **Lane B -> owner (default chosen).** Inventory definitions name members after their
  signature (`Edid`, `Wfir`, `Flld2`); renaming them once meanings are sourced (`ck-wiki`)
  is a breaking change for `kit query`. Default: accept the rename when it happens.
