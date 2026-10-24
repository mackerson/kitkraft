# Support-depth matrix

How far KitKraft supports each record type, as six statuses kept apart (owner ruling
2026-10-15, `docs/foothold/owner-decisions-2026-10-15.md`, "Acceptance criteria"). Schema
presence is never counted as runtime support: a definition, a typed member or a passing unit
test does not make a type game-runtime-tested.

```text
kit coverage --matrix                          the official files in the Data dir
kit coverage --matrix --types LCTN --types TERM
kit coverage --matrix X.esm                    one plugin
kit coverage --matrix --json                   one row object per type
```

Over MCP: the `coverage` tool (or `execute` with `coverage`) with `{"matrix": true}` and the
same optional `plugin`, `plugins`, `types`, `evidence` and `data_dir`. Implementation:
`crates/engine/src/handlers/matrix.rs`; tests there (`matrix_rows_keep_six_statuses_apart`,
`every_evidence_pointer_resolves`), in `crates/engine/src/cmdline.rs` and in
`crates/mcp/src/server.rs` (`coverage_matrix_over_mcp`).

## Statuses

Every cell is `pass`, `fail`, `inconclusive` or `not-run` (AGENTS.md, "Reporting").

| Status | Measured or read | `pass` means |
| ------ | ---------------- | ------------ |
| parsed | measured on the examined plugins | every instance parses: record header, subrecords, a compressed body that decompresses |
| typed-readable | measured | the type has a definition and every instance decodes through it with no unknown-bytes fallback: no subrecord the definition does not name, none that fails its layout, and every record re-encodes byte for byte. The share of payload bytes a layout types is shown beside it (opaque members and opaque fields type none) |
| typed-writable | evidence file | a test writes the type through `new`, `set` or `build` (a reference: `place` with `fields`), reads it back, and `validate` passes |
| CK-differential-tested | evidence file | a Creation Kit differential observation (`docs/ck-observations.md`) |
| game-runtime-tested | evidence file | an in-game observation on the KitKraft Dev profile (`docs/game-observations.md`, or an owner result in `plan/STATUS.md`) |
| save-reload-verified | evidence file | the in-game state survived a save, quit and reload |

A measured status is `not-run` when the examined plugins have no instance of the type. An
evidence status is `not-run` when the evidence file names nothing for the type, and
`inconclusive` when its entry's pointers do not all resolve (never `pass`).

## Evidence file

`docs/schema/evidence.toml`: one `[[entry]]` per type and status, with `result` and
`evidence` pointers: `test: <file>::<fn>`, `doc: <file>#<heading or bold label>`,
`ledger: <entry id>`. The command finds it above the working directory, else in the checkout
`kit` was built from, else takes `evidence`. `every_evidence_pointer_resolves` fails the gate
when a pointer does not resolve. Add a row only with a pointer to the proof; an owner result
that is still pending is not a row.

Seeded 2026-10-15 from what `main` proves:

- typed-writable: LCTN, TERM, PKIN, FACT (`crates/engine/src/handlers/authoring_tests.rs`),
  GLOB (`newform_tests.rs`), WEAP, AMDL, OMOD (`tests/overhaul_build.rs`);
- game-runtime-tested: WEAP (the rename, ledger `game-20261014-01`, and overhaul build 2,
  STATUS "M3.O5 result"), AMDL and OMOD (overhaul build 2), CELL and REFR (the first POI,
  STATUS "M4.O2 result"). CONT is not: the POI's crate is a STAT;
- CK-differential-tested and save-reload-verified: none (the CK sessions are deferred; the
  M4.O2 and O5-15 persistence rows are pending the owner).

## LCTN, TERM, PKIN, FACT (lane D, 2026-10-15)

Over the 14 official files (`kit coverage --matrix --types LCTN --types TERM --types PKIN
--types FACT`, release build, 6 s):

| Type | Records | parsed | typed-readable | typed-writable | CK-diff | game-runtime | save/reload |
| ---- | ------: | ------ | -------------- | -------------- | ------- | ------------ | ----------- |
| FACT | 953 | pass | pass, 98.6 % typed bytes | pass | not-run | not-run | not-run |
| LCTN | 7,316 | pass | pass, 83.4 % | pass | not-run | not-run | not-run |
| PKIN | 15,128 | pass | pass, 78.2 % | pass | not-run | not-run | not-run |
| TERM | 501 | pass | pass, 52.6 % | pass | not-run | not-run | not-run |

Before (main at `1d72018`, `docs/record-parity.md`): all four parsed; typed-readable `pass`
for LCTN only. TERM kept 1,990 subrecords raw in its component blocks (ANAM, BNAM, CNAM,
REFL), PKIN 7,716 (the placement properties, their conditions, REFL), FACT one (FLCS); none
had a typed-writable test.

What keeps the typed share below 100 %: TERM's `VMAD` (script data, every record) and
`REFL`; PKIN's `REFL` (LOD owner and effect sequence components); LCTN's reference lists'
4 unnamed bytes per entry (`Bytes12`/`Bytes16`, unknown upstream); the condition and
component bytes upstream leaves unused. `REFL` is read by a helper in another upstream unit,
which is not an authorized source (ADR 0004).

The proofs:

- `crates/doc/tests/authoring_types.rs` (`authoring_types_full`): every official record of
  the four types decodes through its definition with nothing kept raw and no raw component
  body, and re-encodes byte for byte.
- `crates/doc/tests/components.rs` (`component_layouts_full`): the four component layouts
  the shared block types (animation graph, contact shadow, placement properties,
  reflection) on every official record, each block's name matching its layout.
- `crates/engine/src/handlers/authoring_tests.rs`: a record of each type built with `new`,
  every leaf member of the definition given a value (struct fields one by one through
  `set`'s paths), read back, re-encoded identically, `validate` with no finding; and per
  type a deliberately dangling link (reference lists, the terminal menu, component and
  condition FormIDs, the vendor location, relations) that `validate`'s `formid.dangling`
  reports and `refs` lists as `missing`.
- `crates/engine/tests/clone_sample_corpus.rs` (KK-004, ignored, `--include-ignored`): 100
  official records each, spread evenly by FormID over `Starfield.esm`'s 6,017 LCTN and 368
  TERM, cloned into one patch per type. `clone-sample LCTN: 100 cloned, 100 identical, 0
  explained`; `clone-sample TERM: 100 cloned, 100 identical, 0 explained`. A clone changes
  only the FormID, the `EDID` and the lstrings it inlines from the string tables; every
  other subrecord is byte-identical and the patch validates with 0 errors.

Known limits: `set` paths do not address the elements of a subrecord whose whole payload is
an array (LCTN's reference lists, FACT's `PRIS`): such a member is written whole (`hex:`).
`new` writes one component block per record. Parameters 1 and 2 of conditions whose
parameter is a reference, an actor or a package stay raw bytes (the type byte's alias and
packdata bits decide them; `common/Condition.toml`). `VMAD` object properties are not yet
followed by `refs` or `formid.dangling` (M5.D6).

## COBJ, STAT, REFR, ACHR, CELL, WEAP, CONT (lane D round 4, 2026-10-16)

Over the 14 official files (`kit coverage --matrix --types COBJ --types STAT --types REFR
--types ACHR --types CELL --types WEAP --types CONT`, release build); before is `main` at
`97dc5ee`:

| Type | Records | typed-readable before | after | typed-writable before | after |
| ---- | ------: | --------------------- | ----- | --------------------- | ----- |
| ACHR | 14,731 | fail, 88.6 % | fail, 88.6 % (1,548 unknown: `KSIZ`, `KWDA`, `OPDS`) | not-run | pass |
| CELL | 40,251 | fail, 0.4 % | fail, 0.4 % (4 unknown: `MOD2`) | not-run | pass |
| COBJ | 3,518 | fail, 84.5 % | fail, 84.5 % (154 unknown: `FLTR`, `INTV`) | not-run | pass |
| CONT | 866 | fail, 69.8 % | **pass**, 72.2 % | not-run | not-run |
| REFR | 6,719,725 | fail, 92.5 % | fail, 92.5 % (161,283 unknown) | not-run | pass |
| STAT | 22,296 | fail, 56.3 % | fail, 56.3 % (21,408 unknown) | not-run | pass |
| WEAP | 452 | fail, 68.5 % | **pass**, 68.8 % | pass | pass |

Parsed is `pass` for all seven before and after; CK-differential, game-runtime and
save/reload are unchanged (game `pass` for CELL, REFR and WEAP, the rest `not-run`).

Typed-readable: WEAP kept 111 subrecords raw (`PTCL` 26, `QUPA` 72, `STCP` 13) and CONT
316 (`DCED` 156, `DCSD` 156, `PTCL` 4), all inside base form component blocks. Four
component layouts were added to `common/BfcbBlock.toml` from the pinned
`wbDefinitionsSF1.pas`: `DisplayCase` (L6122-L6139: `DCSD`, 16-byte slot templates;
`DCED`, a count-prefixed list of exclusive slots per template), `QualityUpgrade`
(L6203-L6206: `QUPA`, OMOD FormIDs), `SoundTag` (L6207-L6210 with L5675: `STCP`, an STAG
FormID) and `ParticleSystem` (L6607-L6610: `PTCL`). `PTCL` stays opaque, as `REFL` does:
upstream reads both with a helper in a unit that is not an authorized source (ADR 0004).
`crates/doc/tests/components.rs` (`component_layouts_full`) checks every official block of
the eight layouts (156 display case, 216 quality upgrade, 966 sound tag, 1,545 particle
system blocks) against its component name and for identical re-encoding;
`crates/doc/tests/authoring_types.rs` (`authoring_types_full`) now decodes every WEAP and
CONT record with nothing raw. The other five types still keep raw component data or
top-level signatures no definition names (`docs/record-parity.md`).

Typed-writable, in the pattern of the LCTN to FACT tests above:

- `crates/engine/src/handlers/authoring_tests.rs`: `cobj_new_sets_every_typed_member_and_reads_back`
  and `stat_new_sets_every_typed_member_and_reads_back` (`new`); `cobj_dangling_link_is_reported`
  (`CNAM`, `BNAM`, `JNAM`) and `stat_dangling_link_is_reported` (`SNTP`, `SNBH`, `DEFL`).
- `crates/engine/src/handlers/authoring_world_tests.rs`, on the `place` tests' synthetic
  master: `refr_place_sets_every_typed_member_and_reads_back` and
  `achr_place_sets_every_typed_member_and_reads_back` (`place` with `fields`, new this
  round: the subrecords the fields name are added at their schema slots, then written as
  `set` writes them; `NAME` is `place`'s base, and an ACHR keeps `place`'s `DATA` so its
  position stays in its exterior cell's grid); `cell_new_sets_every_typed_member_and_reads_back`
  and `build_new_writes_an_interior_cell` (`new` and `[[new]]` of a `CELL`, new this round:
  an interior cell, `DATA` flag `Interior` unless given, in the interior block and
  sub-block its FormID gives, `INCC` counted; `a_new_exterior_cell_is_refused`); one
  dangling-link test per type (`XLCN`, `XEZN`, `XLYR`; `LTMP`, `XLCN`, `XCWT`).

Every write test checks that each planned member is placed, nothing is kept raw, the
record re-encodes byte for byte, the field values read back, and `validate` returns no
finding of any rule (so no `layout.undecoded`) with only the whole-opaque members
unchecked. Known limits: a new cell has no references and cannot be named by a `[[place]]`
of the same build; a new exterior cell is refused; what the Creation Kit writes for a new
interior cell is not observed (CK-differential `not-run`).
