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

- typed-writable: LCTN, TERM, PKIN, FACT, CONT, MISC, FLST, ARMO, ARMA, OTFT, ALCH, BOOK,
  KEYM, FLOR, DOOR, LIGH, FURN, MSTT,
  MESG, ENCH, IRES, HAZD, DIAL, INFO, WWED, RSPJ, AVIF, PERK, REGN, TXST, SOUN, WKMF, PCMT, PCBN, PCCN, PNDT, LVLP, LVLN, NPC_, PACK, COLL, MATT, ADDN, SPCH, STND and MOVT (`crates/engine/src/handlers/authoring_tests.rs`), GLOB (`newform_tests.rs`), WEAP,
  AMDL, OMOD (`tests/overhaul_build.rs`);
- game-runtime-tested: WEAP (the rename, ledger `game-20261014-01`, and overhaul build 2,
  STATUS "M3.O5 result"), AMDL and OMOD (overhaul build 2), CELL and REFR (the first POI,
  STATUS "M4.O2 result"). CONT is not: the POI's crate is a STAT;
- CK-differential-tested: CELL and REFR (the first POI, F30 re-exported by the CK and F29
  placed in it, every difference explained: `docs/ck-observations.md` `ck-20261101-04`);
- save-reload-verified: CELL and REFR (the first POI's crate after a save and load,
  unattended: `docs/game-observations.md` `game-20261101-04`). The O5-15 persistence row
  (overhaul build 2) is pending.

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
which is not an authorized source (ADR 0004); since 2026-10-31 it is known not to be
needed, because the payload names its own classes and fields
(`docs/research/reflection-scan-2026-10-31.md`).

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
- `crates/engine/tests/clone_sample_corpus.rs` (KK-014, ignored, `--include-ignored`):
  `a_sample_of_official_packins_clones_identically` and
  `a_sample_of_official_factions_clones_identically` each clone 100 official records,
  spread evenly by FormID over `Starfield.esm`, into one patch per type.
  `clone-sample PKIN: 100 cloned, 100 identical, 0 explained` over Starfield.esm's 11281
  PKIN, in 1154.34 s (about 19 minutes); `clone-sample FACT: 100 cloned, 100 identical,
  0 explained` over its 812 FACT, in 959.72 s (about 16 minutes). A clone changes only the
  FormID, the `EDID` and the lstrings it inlines from the string tables; every other
  subrecord is byte-identical and the patch validates with 0 errors.
- `crates/engine/tests/clone_sample_corpus.rs` (KK-261, ignored, `--include-ignored`):
  `a_sample_of_official_ingestibles_clones_identically` and
  `a_sample_of_official_books_clones_identically` each clone 100 official records, spread
  evenly by FormID over `Starfield.esm`, into one patch per type.
  `clone-sample ALCH: 100 cloned, 100 identical, 0 explained` over Starfield.esm's 353
  ALCH, in 2947.01 s (about 49 minutes, debug build on a shared machine);
  `clone-sample BOOK: 100 cloned, 100 identical, 0 explained` over its 992 BOOK, in
  3645.15 s (about 61 minutes, same). Same claim as above: only the FormID, the `EDID` and
  the inlined lstrings change, and the patch validates with 0 errors. The ALCH sample is
  the first with lstrings of ID 0 (98 of its 100 records, in `DNAM` or `DESC`): the writer
  inlines ID 0 as one NUL (`edit/patch.rs` `inline_lstrings`), and the test's expected
  payload now does the same (it had put two NULs there, against its own doc comment).
- `crates/engine/tests/clone_sample_corpus.rs` (KK-266, ignored, `--include-ignored`):
  `a_sample_of_official_misc_items_clones_identically` and
  `a_sample_of_official_doors_clones_identically` each clone 100 official records, spread
  evenly by FormID over `Starfield.esm`, into one patch per type.
  `clone-sample MISC: 100 cloned, 100 identical, 0 explained` over Starfield.esm's 1319
  MISC, in 2975.58 s (about 50 minutes, debug build on a shared machine);
  `clone-sample DOOR: 100 cloned, 100 identical, 0 explained` over its 339 DOOR, in
  2916.14 s (about 49 minutes, same), script data (`VMAD`) and component blocks
  (`BFCB` .. `BFCE`) copied as stored. Same claim as above: only the FormID, the `EDID` and
  the inlined lstrings change, and the patch validates with 0 errors.
- `crates/engine/tests/clone_sample_corpus.rs` (KK-342, ignored, `--include-ignored`):
  `a_sample_of_official_containers_clones_identically` clones 100 official records, spread
  evenly by FormID over `Starfield.esm`, into one patch.
  `clone-sample CONT: 100 cloned, 100 identical, 0 explained` over Starfield.esm's 707
  CONT, in 427.22 s (about 7 minutes, release build on a shared machine), item entries
  (`CNTO`, `COED`), component blocks and script data (`VMAD`) copied as stored. Same claim
  as above: only the FormID, the `EDID` and the inlined lstrings change, and the patch
  validates with 0 errors.
- `crates/engine/tests/clone_sample_corpus.rs` (KK-343, ignored, `--include-ignored`):
  `a_sample_of_official_keys_clones_identically` clones 100 official records, spread
  evenly by FormID over `Starfield.esm`, into one patch.
  `clone-sample KEYM: 100 cloned, 100 identical, 0 explained` over Starfield.esm's 242
  KEYM, in 492.30 s (about 8 minutes, release build on a shared machine). Same claim as
  above: only the FormID, the `EDID` and the inlined lstrings change, and the patch
  validates with 0 errors.

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
Both are in the self-describing reflection format, readable from the corpus without it
(`docs/research/reflection-scan-2026-10-31.md`).
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

## ACTI, KYWD, TMLM, QUST and WRLD (KK-002, 2026-10-24)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`):

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| ACTI | not-run | pass |
| KYWD | not-run | pass |
| TMLM | not-run | pass |
| QUST | not-run | pass |

- `acti_new_sets_every_typed_member_and_reads_back`, `kywd_new_sets_every_typed_member_and_reads_back`
  and `tmlm_new_sets_every_typed_member_and_reads_back`: `new` with every typed leaf member
  of the definition, read back byte for byte, `validate` with no finding.
- `qust_new_sets_every_typed_member_and_reads_back` (KK-220) and
  `qust_contact_shadow_new_sets_every_typed_member_and_reads_back` (KK-220): `new` with
  every typed leaf member of the definition, read back byte for byte, `validate` with no
  finding, the first on the `PlanetContentProperties` component layout (the block's
  condition is the first of the record's ten `CTDA` slots) and the second on the
  `ContactShadow` layout (the block's condition slot is left unplanned and out of `new`'s
  slot list, so every planned `CTDA` lands in its own condition group, the collection
  alias's match conditions included).
- `acti_dangling_link_is_reported` (`SNTP`, `SNBH`, `DEFL`), `kywd_dangling_link_is_reported`
  (`DATA`, the attached resource), `tmlm_dangling_link_is_reported` (`TNAM`, `BNAM`,
  `XLOC.Key`) and `qust_dangling_link_is_reported` (`QTYP`, `LNAM`, `GNAM`, `QSRC`, a
  condition's `Parameter1`): each link is reported by `validate`'s `formid.dangling` and
  listed as `missing` by `refs`.

KYWD and TMLM have no base form component block of their own, so the layout and component
name in their tests are inert; every other typed member of the definition is written and
read back.

WRLD stays out of the list: `new` refuses a WRLD (a worldspace roots exterior cells that
are not written yet), and its read-back and dangling tests are `#[ignore]`d. QUST's slot
misalignment was fixed in KK-220: the plan now walks the chosen layout's own groups, and
`new`'s slot list holds the block's slots of only the layout the record's fields select,
so the k-th `CTDA` lands in the k-th `CTDA` slot for every slot the definition has (the
slot rule pinned by
`edit::scaffold::tests::a_repeated_signature_counts_every_slot_of_a_repeating_group`).

## CONT, MISC, FLST and LVLI (KK-017, 2026-10-27)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`), each on the
`ContactShadow` component layout:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| CONT | not-run | pass |
| MISC | not-run | pass |
| FLST | not-run | pass |
| LVLI | not-run | pass (2026-11-01, below) |

- `cont_new_sets_every_typed_member_and_reads_back`,
  `misc_new_sets_every_typed_member_and_reads_back` and
  `flst_new_sets_every_typed_member_and_reads_back`: `new` with every typed leaf member
  of the definition, read back byte for byte, `validate` with no finding.
- `cont_dangling_link_is_reported` (a CNTO entry's item, `SNTP`, `SNBH`),
  `misc_dangling_link_is_reported` (`SNTP`, `SNBH`, `FIMD`) and
  `flst_dangling_link_is_reported` (`LNAM`, `ANAM`): each link is reported by
  `validate`'s `formid.dangling` and listed as `missing` by `refs`.

LVLI joined the list on 2026-11-01 (typed-writable `not-run` to `pass`):
`lvli_new_sets_every_typed_member_and_reads_back` was `#[ignore]`d here because `new`'s
member order then held every layout of the BFCB block, whose `PlanetContentProperties`
layout has the block's only condition slot ahead of the record's top-level `Conditions`
group, so the list's and the entries' conditions landed one slot early and the entries'
conditions were never written. KK-220's layout-aligned slots (`new` holds only the
layout the fields select; `FLCS` selects `ContactShadow`) fixed it; the test is
un-ignored and passes. The rule now lives in `edit::scaffold::record_order`, which `new`
and `set`'s `add_slots` share, and it also covers a record that selects no layout: the
block then holds none, so a new LVLI with a list condition and an entry condition and no
component puts each in its own group
(`a_new_lvli_without_a_component_puts_each_condition_in_its_own_group`; before, `new`
refused it for the missing `BFCB`; rule pinned by
`edit::scaffold::tests::a_record_holds_only_the_component_layout_its_occurrences_select`).
Its dangling-link test, `lvli_dangling_link_is_reported` (`LVLO.Item`, `DEFL`, `SNBH`),
passes.

## ARMO, ARMA and OTFT (KK-021, 2026-10-27)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`), each on the
`ContactShadow` component layout:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| ARMO | not-run | pass |
| ARMA | not-run | pass |
| OTFT | not-run | pass |

- `armo_new_sets_every_typed_member_and_reads_back`,
  `arma_new_sets_every_typed_member_and_reads_back` and
  `otft_new_sets_every_typed_member_and_reads_back`: `new` with every typed leaf member
  of the definition, read back byte for byte, `validate` with no finding. ARMO's AVSG
  and AFSG Wwise GUID subrecords are structs of opaque fields only, which `validate`
  counts as unchecked subrecords, so the read-back allows them unchecked; ARMA's biped
  slots (`BO64`) are a u64 flags member (KK-006), written whole as the other flags
  members are.
- `armo_dangling_link_is_reported` (the armor addon `MODL`, the keywords `KWDA`),
  `arma_dangling_link_is_reported` (the race `RNAM`, the additional races `MODL`) and
  `otft_dangling_link_is_reported` (the outfit items `INAM`): each link is reported by
  `validate`'s `formid.dangling` and listed as `missing` by `refs`.

## ALCH, BOOK and KEYM (KK-258, 2026-11-14)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`), ALCH and BOOK on the `ContactShadow`
component layout; KEYM has no base form component block, so the layout names are inert:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| ALCH | not-run | pass |
| BOOK | not-run | pass |
| KEYM | not-run | pass |

- `alch_new_sets_every_typed_member_and_reads_back` (46 subrecords),
  `book_new_sets_every_typed_member_and_reads_back` (38) and
  `keym_new_sets_every_typed_member_and_reads_back` (32): `new` with every typed leaf
  member of the definition, read back byte for byte, `validate` with no finding. BOOK's
  `CNAM` is whole-opaque, which `validate` counts as its one unchecked subrecord; the
  `VMAD` kept opaque in ALCH and KEYM is not written.
- `alch_dangling_link_is_reported` (the equipment type `ETYP`, the effect data's
  addiction spell `ENIT.Addiction`, an effect's base effect `EFID`),
  `book_dangling_link_is_reported` (`DEFL`, `FIMD`, the inventory art `INAM`, the scene
  `GNAM`) and `keym_dangling_link_is_reported` (`FIMD`, the keywords `KWDA`): each link is
  reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.

## FLOR, DOOR and LIGH (KK-259, 2026-11-14)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`), each on the `ContactShadow` component
layout:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| FLOR | not-run | pass |
| DOOR | not-run | pass |
| LIGH | not-run | pass |

- `flor_new_sets_every_typed_member_and_reads_back` (47 subrecords),
  `door_new_sets_every_typed_member_and_reads_back` (39) and
  `ligh_new_sets_every_typed_member_and_reads_back` (38): `new` with every typed leaf
  member of the definition, read back byte for byte, `validate` with no finding. DOOR's
  three whole-opaque members (`ONAM`, `CNAM`, `NVNM`) are its unchecked subrecords.
  LIGH keeps `FULL` opaque in its definition, but `validate` reads a `FULL` with only
  opaque bytes in the schema through the engine's lstring fallback, so the test
  (`writes_and_reads_back_with`, `FULL` named) expects it checked: LIGH has no
  unchecked subrecord.
- `flor_dangling_link_is_reported` (the ingredient `PFIG`, the maximum-count global
  `FMAG`, the explosion `FLXP`), `door_dangling_link_is_reported` (the snap template
  `SNTP`, the native terminal `NTRM`, the keywords `KWDA`) and
  `ligh_dangling_link_is_reported` (the default layer `DEFL`, the lens `LNAM`, `KWDA`):
  each link is reported by `validate`'s `formid.dangling` and listed as `missing` by
  `refs`.

## FURN, MSTT and MESG (KK-260, 2026-11-14)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`), each on the `ContactShadow` component
layout:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| FURN | not-run | pass |
| MSTT | not-run | pass |
| MESG | not-run | pass |

- `furn_new_sets_every_typed_member_and_reads_back` (51 subrecords),
  `mstt_new_sets_every_typed_member_and_reads_back` (34) and
  `mesg_new_sets_every_typed_member_and_reads_back` (20): `new` with every typed leaf
  member of the definition, read back byte for byte, `validate` with no finding. FURN's
  marker files (`NNAM`, whole-opaque) are its one unchecked subrecord; the `VMAD` the
  three definitions keep raw is not written.
- `furn_dangling_link_is_reported` (the snap template `SNTP`, the native terminal
  `NTRM`, the water `WTFM`), `mstt_dangling_link_is_reported` (`SNTP`, the snap behavior
  `SNBH`, the default layer `DEFL`) and `mesg_dangling_link_is_reported` (the icon `INAM`,
  the owner quest `QNAM`, the reference `DODT`): each link is reported by `validate`'s
  `formid.dangling` and listed as `missing` by `refs`.

## ENCH, IRES and HAZD (KK-265, 2026-11-14)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`), ENCH and HAZD on the `ContactShadow`
component layout; IRES has no base form component block, so the layout names are inert:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| ENCH | not-run | pass |
| IRES | not-run | pass |
| HAZD | not-run | pass |

- `ench_new_sets_every_typed_member_and_reads_back` (18 subrecords),
  `ires_new_sets_every_typed_member_and_reads_back` (14) and
  `hazd_new_sets_every_typed_member_and_reads_back` (24): `new` with every typed leaf
  member of the definition, read back byte for byte, `validate` with no finding and no
  unchecked subrecord (the `VMAD` ENCH and HAZD keep raw is not written).
- `ench_dangling_link_is_reported` (the effect data's base enchantment
  `ENIT.BaseEnchantment` and worn restrictions `ENIT.WornRestrictions`, an effect's base
  effect `EFID`), `ires_dangling_link_is_reported` (the surface item `FNAM`, the resource
  actor value `NAM1`, the resource item `NAM2`, the production interval `NAM3`) and
  `hazd_dangling_link_is_reported` (the snap template `SNTP`, the default layer `DEFL`,
  the image space modifier `MNAM`): each link is reported by `validate`'s
  `formid.dangling` and listed as `missing` by `refs`.

## DIAL and INFO (KK-277, 2026-11-15)

Typed-writable, in the pattern of the tests above, for two types that live in nested
groups: each record is written with `new` under a `parent` (`docs/design/dialogue.md`) on
the dialogue fixture of `crates/engine/src/handlers/dialogue_tests.rs` (`Quest.esm`: a
quest, a topic, a response), the parent carried into the patch:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| DIAL | not-run | pass |
| INFO | not-run | pass |

- `dial_new_sets_every_typed_member_and_reads_back` (14 subrecords; `QNAM` is written
  from the parent, so the plan keeps the member and the test checks it holds the quest)
  and `info_new_sets_every_typed_member_and_reads_back` (60 subrecords, 3 whole-opaque):
  read back byte for byte, `validate` with no finding.
- `dial_dangling_link_is_reported` (the branch `BNAM`, keyword `KNAM`, affinity event
  `ANAM`) and `info_dangling_link_is_reported` (the shared info `DNAM`, info group `GNAM`,
  a response's emotion `TRDA.Emotion` and voice type `TROT.VoiceType`): each link is
  reported by `formid.dangling` and listed as `missing` by `refs`.
- Unverified: the group layout (type 10 quest children, type 7 topic children) is read off
  the official files only; no Creation Kit plugin with quest children is observed
  (`docs/gaps.md`).

## WWED (KK-291, 2026-11-15)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`); WWED has no base form component block,
so the layout names are inert:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| WWED | not-run | pass |

- `wwed_new_sets_every_typed_member_and_reads_back` (4 subrecords): `new` with every
  typed leaf member of the definition, read back byte for byte, `validate` with no
  finding; the two Wwise event GUIDs (`WSED` and `WTED`, whole-opaque) are its unchecked
  subrecords.
- `wwed_dangling_link_is_reported` (the condition `CNAM`, a FormID of a `CNDF`): the link
  is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.

## RSPJ, AVIF, PERK, REGN, TXST, SOUN and WKMF (KK-300 to KK-305, 2026-11-15)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`). RSPJ and TXST on the `ContactShadow` component layout; AVIF, PERK, REGN, SOUN and WKMF have no base form component block, so the layout names are inert:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| RSPJ | not-run | pass |
| AVIF | not-run | pass |
| PERK | not-run | pass |
| REGN | not-run | pass |
| TXST | not-run | pass |
| SOUN | not-run | pass |
| WKMF | not-run | pass |

- KK-300: `rspj_new_sets_every_typed_member_and_reads_back` (18 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. `rspj_dangling_link_is_reported` (the workbench keyword `BNAM`, the created item `CNAM`, the category keyword `KNAM`, the required research `RNAM`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-301: `avif_new_sets_every_typed_member_and_reads_back` (9 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. No dangling-link test: AVIF has no FormID member.
- KK-302: `perk_new_sets_every_typed_member_and_reads_back` (39 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and 3 whole-opaque subrecords unchecked. `perk_dangling_link_is_reported` (the restriction `TNAM`, the training `UNAM` and the bonus perks `RNAM`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-303: `regn_new_sets_every_typed_member_and_reads_back` (6 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. `regn_dangling_link_is_reported` (the weather of a weather type `RDWT`, written whole): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-304: `txst_new_sets_every_typed_member_and_reads_back` (16 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. No dangling-link test: TXST has no FormID member outside the base form component block.
- KK-305: `soun_new_sets_every_typed_member_and_reads_back` (8 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. No dangling-link test: SOUN has no FormID member.
- KK-305: `wkmf_new_sets_every_typed_member_and_reads_back` (6 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. `wkmf_dangling_link_is_reported` (the keyword list `WMKA`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.

## PCMT, PCBN, PCCN, PNDT, LVLP and LVLN (KK-306 to KK-311, 2026-11-15)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`). PCMT, PCBN, PCCN, PNDT, LVLP and LVLN on the `ContactShadow` component layout:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| PCMT | not-run | pass |
| PCBN | not-run | pass |
| PCCN | not-run | pass |
| PNDT | not-run | pass |
| LVLP | not-run | pass |
| LVLN | not-run | pass |

- KK-306: `pcmt_new_sets_every_typed_member_and_reads_back` (13 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. `pcmt_dangling_link_is_reported` (the nodes `PCCB` and the keyword list `KWDA`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-307: `pcbn_new_sets_every_typed_member_and_reads_back` (17 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. `pcbn_dangling_link_is_reported` (the count and distribution curves `NAM3` and `NAM4`, the nodes `PCCB`, the parent and previous nodes `PRTN` and `PRVN`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-308: `pccn_new_sets_every_typed_member_and_reads_back` (9 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. `pccn_dangling_link_is_reported` (the content `PCCC`, the keyword list `KWDA`, the parent and previous nodes `PRTN` and `PRVN`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-309: `pndt_new_sets_every_typed_member_and_reads_back` (26 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. `pndt_dangling_link_is_reported` (the surface tree `FNAM` and the atmosphere `INAM.Atmosphere`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-310: `lvlp_new_sets_every_typed_member_and_reads_back` (27 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and no unchecked subrecord. `lvlp_dangling_link_is_reported` (the chance-none global `LVLG` and a leveled entry's pack-in `LVLO.PackIn`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-311: `lvln_new_sets_every_typed_member_and_reads_back` (29 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and 1 whole-opaque subrecord unchecked. `lvln_dangling_link_is_reported` (the chance-none global `LVLG` and a leveled entry's actor `LVLO.Npc`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.

## NPC_ and PACK (KK-312 and KK-313, 2026-11-15)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`). NPC_ and PACK on the `ContactShadow` component layout:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| NPC_ | not-run | pass |
| PACK | not-run | pass |

- KK-312: `npc_new_sets_every_typed_member_and_reads_back` (116 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and two unchecked subrecords, the whole-opaque `NAM5` and `ONA2`, whose race override blocks are an opaque tail of no fixed size (`validate` cannot rule out a FormID there, `edit::patch::has_opaque`; the test names it through `writes_and_reads_back_with_opaque_tail`, and the count stays an exact equality). `npc_dangling_link_is_reported` (the race `RNAM`, the class `CNAM`, the voice type `VTCK` and the default layer `DEFL`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.
- KK-313: `pack_new_sets_every_typed_member_and_reads_back` (54 subrecords): `new` with every typed leaf member of the definition, read back byte for byte, `validate` with no finding and 1 whole-opaque subrecord unchecked. `pack_dangling_link_is_reported` (the combat style `CNAM`, the owner quest `QNAM`, the animation flavor `FLAV` and the idle list `IDLA`): each link is reported by `validate`'s `formid.dangling` and listed as `missing` by `refs`.

## COLL, MATT and ADDN (KK-340, 2026-11-17)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`). COLL, MATT and ADDN on the
`ContactShadow` component layout:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| COLL | not-run | pass |
| MATT | not-run | pass |
| ADDN | not-run | pass |

- `coll_new_sets_every_typed_member_and_reads_back` (10 subrecords),
  `matt_new_sets_every_typed_member_and_reads_back` (12) and
  `addn_new_sets_every_typed_member_and_reads_back` (15, the whole-opaque `PSDF`
  unchecked): `new` with every typed leaf member of the definition, read back byte for
  byte, `validate` with no finding.
- `coll_dangling_link_is_reported` (the collides-with layers `CNAM`, a FormID array),
  `matt_dangling_link_is_reported` (the parent material `PNAM`, the impact data set
  `HNAM`) and `addn_dangling_link_is_reported` (the light `LNAM`): each link is reported
  by `validate`'s `formid.dangling` and listed as `missing` by `refs`.

## SPCH, STND and MOVT (KK-341, 2026-11-17)

Typed-writable, in the pattern of the LCTN to FACT tests above
(`crates/engine/src/handlers/authoring_tests.rs`). SPCH and STND on the `ContactShadow`
component layout; MOVT has no base form component block, so the layout names are inert:

| Type | typed-writable before | after |
| ---- | --------------------- | ----- |
| SPCH | not-run | pass |
| STND | not-run | pass |
| MOVT | not-run | pass |

- `spch_new_sets_every_typed_member_and_reads_back` (13 subrecords),
  `stnd_new_sets_every_typed_member_and_reads_back` (27) and
  `movt_new_sets_every_typed_member_and_reads_back` (7): `new` with every typed leaf
  member of the definition, read back byte for byte, `validate` with no finding.
- `spch_dangling_link_is_reported` (the quest `SPQU`, the scenes `SPMA`, a FormID array)
  and `stnd_dangling_link_is_reported` (the snap template `SNTP`, the adjacent snap node
  `NNAM`, the art object `ANAM`): each link is reported by `validate`'s `formid.dangling`
  and listed as `missing` by `refs`. MOVT has no FormID member, so it has no dangling
  test.
