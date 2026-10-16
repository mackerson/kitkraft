# Foothold gap matrix (M4-accurate)

Reconciles the owner's Foothold capability inventory (`docs/foothold/engineering-surface.md`,
sections A to I and K) with what `main` proves, as the owner's 2026-10-15 decisions ask
(priority F, "Deliverable"). Established on `main` at `fcb443a` on 2026-10-15 from the
tests in `crates/*/tests` and the handler tests, `kit schema` and `kit coverage` (release
build of that commit), `docs/record-parity.md`, the two ledgers and `plan/STATUS.md`.
Typed-readable, the unknown signatures and the owner-report cells were rechecked in the
review fix round the same day (`bin\kit.exe schema --type <TYPE>`, `docs/record-parity.md`).
**Checked at `5d03d01` on 2026-10-16** (M5.F2 and M5.F4, lane F). The release `kit.exe`
(SHA-256 `4fa997f8...`) ran `kit coverage --matrix` (key MX) over the 14 official files.
Rows touched since `fcb443a`: the owner-report game cells now cite their ledger entries
(`game-20261015-02`, `-03`); `LCTN`, `TERM`, `PKIN` and `FACT` are typed-writable
(`24ac42d`); conditions and component blocks are partly typed (`24ac42d`); the
`layout.undecoded` rule (`6977793`); the Papyrus compiler oracle (`f20e39e`); the register
and compatibility rows (M5.F1, M5.F3). Every other cell is as of `fcb443a`, except the
rows lane D round 4 refreshed from MX4 on 2026-10-16 (`CELL`, `REFR`, `ACHR`, `STAT`,
`CONT`, `COBJ`, `WEAP` and the component-block capability).
Nothing here is a plan: the tickets that close the gaps are M5 in `plan/execution-plan.md`.
Update a cell only with the commit, test or ledger entry that changes it.

## Statuses

Six statuses, never collapsed into one (decisions, "Acceptance criteria"; AGENTS.md
"Reporting"). Schema presence is not feature support.

| Column | Meaning | Values |
| ------ | ------- | ------ |
| Parsed | Every corpus record of the type is read and written back byte-identically (unedited) | yes, no |
| Typed-readable | The schema types its members at `corpus` confidence and every corpus occurrence decodes, with 0 re-encode conflicts | yes; partial (a member is below `corpus` confidence, `reference` or `container-reference`, counted as `kit schema --type <TYPE>` prints it; or a payload the feature needs is opaque or unknown); no (inventory or lstring-only definition, or none) |
| Typed-writable | A KitKraft write of the type (`set`, `new`, `clone`, `place`, `build`) is covered by a test or a built, validated, roundtripped recipe | yes; partial; untested (the generic path accepts it, nothing proves it); no (refused, or members opaque) |
| CK-diff | A CK-authored or CK-re-exported fixture compared against KitKraft (`docs/ck-observations.md`) | pass, fail, not-run, inconclusive |
| Game | Observed in the running game on `KitKraft Dev` and recorded in `docs/game-observations.md` | pass, fail, not-run, inconclusive. An owner result counts once a ledger entry holds it with the plugins' hashes (M5.F4, 2026-10-16: `game-20261015-02` and `-03`, grade "owner report"); a result whose hashes cannot be recovered is `inconclusive` |
| Save/reload | The game observation survives save, quit and reload (persistence protocol steps 1 to 4) | pass, fail, not-run, inconclusive |

`n/a`: the status does not apply to that row.

## Evidence keys

| Key | Evidence |
| --- | -------- |
| RT | Corpus roundtrip: all 14 official files byte-identical (`crates/esm/tests/corpus.rs` `corpus_full_roundtrip`; STATUS "Done", M1.6); compressed records and `XXXX` (`corpus_small_compressed_and_xxxx`); 136,741 compressed records decoded, 0 failures (`docs/record-parity.md`) |
| SC | `kit schema` at `fcb443a`: 96 definitions, 2,209 members (2,005 typed, 204 opaque), confidence `corpus` 1,957, `reference` 250, `container-reference` 2, **`ck-diff` 0, `verified` 0**. At `5d03d01` (2026-10-16): 96 definitions, 3,743 subrecord definitions (3,421 typed, 322 opaque), `corpus` 3,491, `reference` 250, `container-reference` 2, still no `ck-diff` or `verified`. Most of the rise is the shared component group (`common/BfcbBlock.toml`, `24ac42d`) now counted under every type that uses it, so the per-type "typed/opaque" pairs below are `fcb443a`'s; the `reference` counts (ref N) are unchanged (rechecked for `LCTN` 8, `TERM` 19, `TMLM` 0, `PKIN` 0, `FACT` 6). Per type "typed/opaque, ref N" below quotes `kit schema --type <TYPE>` (N counts every subrecord definition at `reference` or `container-reference`, nested ones included; rechecked 2026-10-15 with the `bin\kit.exe` build). Any N above 0 makes Typed-readable `partial` |
| PAR | `docs/record-parity.md` (generated): 0 decode failures and 0 re-encode conflicts over the corpus; `kit coverage SFBGS00D.esm` on 2026-10-15: 434,991 records, 0 failed, 0 re-encode conflicts, decoded by confidence `ck-diff` 0, `verified` 0. "Unknown sigs" below lists every corpus signature the type's definition does not name (`docs/record-parity.md`, "Subrecords outside the definitions"); a row without the phrase has none |
| W1 | `crates/engine/src/handlers/newform_tests.rs`; `out\w1\KitKraftW1.esm` (`new GLOB`, `clone` of STAT `Starfield.esm:1E4FBD`; SHA-256 `5d122dd3...`; STATUS "Done", M4.E1) |
| PL | `crates/engine/src/handlers/place_tests.rs`; `crates/engine/tests/place_corpus.rs` (1,000 corpus references recomposed byte-identically); `crates/esm/tests/rebuild.rs` (M4.A2 rebuild gate) |
| POI | `docs/poi/first-poi.md`: `KitKraftFirstPOI.esm` (SHA-256 `13c22b48...`), one persistent `REFR` of a vanilla STAT in the carried cell `CityNewAtlantisApartmentTEMPLATE`; validate 0/0, roundtrip identical |
| OVH | `crates/engine/tests/overhaul_build.rs`; `docs/overhaul/build-2.md` (21 overrides: WEAP 13, AMDL 4, OMOD 4) |
| REN | `crates/engine/tests/smoke_rename.rs` (WEAP `FULL` rename) |
| REC | Recipes built, validated 0/0 and roundtripped identical (STATUS "Done", lane E round 5): `docs/recipes/workshop.md` (clone STAT + clone COBJ with `CNAM = "new:coffee"`), `fabricator.md` (clone COBJ), `ownership.md` (`CELL` `XOWN`), `actors.md` (`ACHR`); test `field_values_name_new_forms_of_the_build` |
| SCR | `docs/recipes/scripts.md`: `crates/engine/tests/vmad_corpus.rs` (release, `#[ignore]`d by default: all 12,857 official `VMAD`s re-encode identically; attaching keeps vanilla scripts and fragments); `crates/engine/src/handlers/scripts_tests.rs` (`set`, `new`, `place` attach scripts; the two papyrus rules); `KitKraftScripts.esm` (SHA-256 `b20cf577...`) and `HelloOnLoad.pex` compiled by PapyrusCompiler 4.7.0.5 |
| G-REN | `docs/game-observations.md` entry `game-20261014-01` (rename, O2-01 **pass**) |
| G-OVH | Ledger entry `game-20261015-02` (M5.F4, from `plan/STATUS.md` "M3.O5 result (2026-10-15)"): owner report, C1-C7 and C9-C14 pass; `KitKraftGunOverhaul.esm` SHA-256 `5e2f84e7...` recovered from the installed copy, the build output and the package manifest; game 1.16.244.0 from the session's save headers. O5-15 (save/reload) not run |
| G-POI | Ledger entry `game-20261015-03` (M5.F4, from `plan/STATUS.md` "M4.O2 result (2026-10-15)" and commit `a5f08c9`): owner report, the crate reached with `player.moveto 04001000` (rows O2-05, O2-06 pass); `KitKraftFirstPOI.esm` SHA-256 `13c22b48...` recovered from the installed copy (installed by hand, no package manifest); slot `04` confirmed by the session's saves. O2-08 (save/reload) not run, "pending owner confirmation" |
| MX | `kit coverage --matrix` at `5d03d01` (2026-10-16, 14 official files; statuses other than parsed and typed-readable read from `docs/schema/evidence.toml`): typed-writable `pass` for `LCTN`, `TERM`, `PKIN`, `FACT`, `GLOB`, `WEAP`, `AMDL`, `OMOD`; `not-run` for every other type, `TMLM` and `COBJ` included, because the evidence file names no test for them (the "yes" cells below for `COBJ`, `STAT`, `REFR`, `ACHR`, `CELL` rest on REC, W1 and PL); game `pass` for `WEAP`, `AMDL`, `OMOD`, `CELL`, `REFR` (rerun for these types the same day). Its typed-readable column is a byte share and is not this matrix's definition |
| MX4 | `kit coverage --matrix --types COBJ --types STAT --types REFR --types ACHR --types CELL --types WEAP --types CONT` on lane D round 4's branch (base `97dc5ee`, release build, 14 official files, 2026-10-16). Before: typed-readable `fail` for all seven (WEAP 68.5 %, 111 unknown: `PTCL`, `QUPA`, `STCP`; CONT 69.8 %, 316 unknown: `DCED`, `DCSD`, `PTCL`), typed-writable `pass` for `WEAP` only. After: typed-readable `pass` for `WEAP` (68.8 %) and `CONT` (72.2 %), from four component layouts added to `common/BfcbBlock.toml` (`crates/doc/tests/components.rs`, `authoring_types.rs` with WEAP and CONT); typed-writable `pass` for `COBJ`, `STAT` (`authoring_tests.rs`), `REFR`, `ACHR`, `CELL` (`authoring_world_tests.rs`), each `new` (or `place` with `fields`) setting every typed leaf member, re-read identically, `validate` with no finding, plus a dangling-link test per type |
| LU | `validate` rule `layout.undecoded` (error, `6977793`; `docs/validate.md`): a payload that does not decode under its layout is reported, and `set`, `new`, `clone` and `build` refuse to write one; 0 findings on the 14 official files (`validate_corpus_official_files`) |
| PAPY | `cargo xtask oracle papyrus` (`f20e39e`; `docs/oracles.md` "Papyrus compiler oracle"): `kit papyrus compile` against `PapyrusCompiler.exe` 4.7.0.5 run by hand, 8 scripts semantically equivalent in plain and `-op` mode, none byte-identical (the compiler's table order changes per run); three disagreements fixed (P1..P3). The compiler was run by hand, not by the CK |
| CK0 | `docs/ck-observations.md` "Ledger": "No entries yet". No CK fixture exists in the repo or `observe\`. M3.O1 and M4.O1 were deferred by the owner (STATUS "Decision records") |

Every row's CK-diff is `not-run` (CK0, and SC/PAR show no member at `ck-diff`). Save/reload
is `not-run` everywhere: no session has recorded the persistence protocol.

## Record families

One row per record type the inventory names (sections B to G and the matrix in section
I), plus the M3 baseline types.

| Type | Parsed | Typed-readable | Typed-writable | CK-diff | Game | Save/reload | Evidence and limits |
| ---- | ------ | -------------- | -------------- | ------- | ---- | ----------- | ------------------- |
| `TES4` | yes | partial | yes | not-run | pass | not-run | RT; SC 8/3, container-reference 2 (`INTV` TagifyCount among them; `kit schema` ranks it with `reference`); writer keeps `HEDR` count and next object ID, `INCC`, `ONAM` by the corpus rules (`crates/engine/tests/writing_rules.rs`, PL). Game: G-REN (ledger entry `game-20261014-01`: the rename plugin's KitKraft-written header loaded) and G-POI (`game-20261015-03`). Limits: header stamps' meaning unknown (POI); `ONAM` order is the writer's (`docs/gaps.md`) |
| `GRUP`, compressed records, `XXXX` | yes | n/a | yes | not-run | pass | not-run | RT; computed cell and world paths pass the rebuild gate (PL); edited compressed records recompress identically for 99.88% (`docs/gaps.md`, "Recompression parity"). Game: the POI's group path (G-POI). Limits: what the CK writes for a new plugin's groups is not observed (`docs/gaps.md`) |
| Master flags and scales | yes | n/a | partial | not-run | pass | not-run | Full, small, medium and blueprint flags read as independent dimensions (`docs/load-order.md`; corpus has small `SFBGS004`, `SFBGS047`, medium `SFBGS003`, `SFBGS006`, blueprint `BlueprintShips-*`). Writes: full scale only; new forms in small or medium plugins refused (`place_tests.rs` `a_small_master_refuses_new_forms`). Game: one full slot (`04001000`) seen (G-POI); no `FE`/`FD` ID checked (`docs/gaps.md`) |
| `LCTN` | yes | partial | yes | not-run | not-run | not-run | SC 38/2 (`VMAD`, `NAM0` opaque), ref 8 (`VMAD`, `NAM0`, the four removed lists `RCPR`, `RCUR`, `RCUN`, `RCSR`, and the world-cell lists `ACEC`, `RCEC`; the added and current persistent-ref lists `ACPR`, `LCPR` are `corpus`); PAR 29 of 29 seen signatures typed; read by `kit planet` and `refs`. Writes (`24ac42d`, MX): `authoring_tests.rs` `lctn_new_sets_every_typed_member_and_reads_back` and the dangling-link tests; typed-readable on every official record (`crates/doc/tests/authoring_types.rs`). A payload that does not decode is refused (LU). Not in game; no `LocTypeOutpost` location written (FTH-EXP-01) |
| `CELL` | yes | partial | yes | not-run | pass | not-run | SC 33/2, ref 4 (`VMAD`, `XILL`, `XRNK` among them); unknown sig `MOD2` (4: MX4 typed-readable `fail`, 0.4 % typed bytes); `unordered` mode (STATUS M4.BX1). Writes: carried copy of a vanilla cell (PL, POI), `XOWN` override (REC ownership, not run in game), and since lane D round 4 a new interior cell through `new` and `build` `[[new]]`, written in the interior block and sub-block its FormID gives with `INCC` counted (`authoring_world_tests.rs` `cell_new_sets_every_typed_member_and_reads_back`, `build_new_writes_an_interior_cell`, `cell_dangling_link_is_reported`; MX4 typed-writable `pass`). New exterior cells refused (`new.cell-exterior`); a `[[place]]` cannot name a new cell of the same build. Game: the carried apartment cell (G-POI, `game-20261015-03`); no new cell run in game; save/reload row O2-08 not run |
| `WRLD` | yes | partial | partial | not-run | not-run | not-run | SC 38/2, ref 7; unknown sigs `CITC`, `CTDA`, `NAM1`, `NAM8`, `SNAM`, `UNAM`, `VNAM`, `XNAM`, `YNAM`. Writes: carried copy for an exterior placement (`place_tests.rs` `world_positions_pick_the_exterior_or_the_persistent_cell`); new worldspaces refused. No exterior plugin run in game |
| `REFR` | yes | partial | yes | not-run | pass | not-run | SC 93/7, ref 5 (`XRGB`, `XLRL`, `XRFE`, `XFVC`, `XLTW`); unknown sigs `FLTR`, `INTV`, `KSIZ`, `KWDA`, `OPDS`, `PNAM`, `SNAM`, `VLMS` (8, `KSIZ`, `KWDA`, `VLMS`, `OPDS` only inside BFCB component blocks; MX4 typed-readable `fail`, 161,283 occurrences, 92.5 %); PL, POI. Writes (lane D round 4): `place` with `fields` sets every typed leaf member but `NAME` (`authoring_world_tests.rs` `refr_place_sets_every_typed_member_and_reads_back`, `refr_dangling_link_is_reported`; MX4 typed-writable `pass`). Game: the POI crate (G-POI). Limits: flag 0x800 (initially disabled) only with `allow_reference` (`docs/gaps.md`); XSL1, XNSE, XTV2 raw |
| `ACHR` | yes | partial | yes | not-run | not-run | not-run | SC 42/3, ref 11; unknown sigs `KSIZ`, `KWDA`, `OPDS` (MX4 typed-readable `fail`, 1,548 occurrences, 88.6 %); PL (`ACHR` for an `NPC_` base), REC actors (O4-03 pending owner). Writes (lane D round 4): `place` with `fields` into an exterior cell with a navmesh sets every typed leaf member but `NAME` and `DATA` (`authoring_world_tests.rs` `achr_place_sets_every_typed_member_and_reads_back`, `achr_dangling_link_is_reported`; MX4 typed-writable `pass`) |
| `ACTI` | yes | partial | untested | not-run | not-run | not-run | SC 45/2, ref 9; unknown sigs `ANAM`, `BNAM`, `CNAM`, `ENAM`, `FLCS`, `PTCL`, `REFL`, `STCP`. The scripts recipe places a `REFR` of the vanilla `ACTI` `ActivatorSwitch01` and writes `VMAD` on the reference; no `ACTI` record is written |
| `KYWD` | yes | partial | untested | not-run | not-run | not-run | SC 9/2 (FNAM opaque), ref 1 (`VMAD`); unknown sigs `WAIM`, `WFIR`. Read as the outpost and workbench categories (REC workshop, fabricator) |
| `TERM` | yes | partial | yes | not-run | not-run | not-run | SC 57/2, ref 19 (conditions, `NTRM` menu, sounds); `WBDT` and `VMAD` opaque (`docs/gaps.md`, items family); unknown sigs `ANAM`, `BNAM`, `CNAM`, `REFL`. Writes (`24ac42d`, MX): `authoring_tests.rs` `term_new_sets_every_typed_member_and_reads_back`. `TMLM` (menus) SC 24/1, ref 0; MX shows `TMLM` typed-writable `not-run` (no test cited for it) |
| `PKIN` | yes | partial | yes | not-run | not-run | not-run | SC 22/1, ref 0; writes (`24ac42d`, MX): `authoring_tests.rs` `pkin_new_sets_every_typed_member_and_reads_back`, the placement properties of its component blocks typed (`PlanetContentProperties`, `crates/doc/tests/components.rs`); unknown sigs `ANAM`, `BNAM`, `CITC`, `CTDA`, `NAM1`..`NAM9`, `REFL`, `UNAM`, `WNAM`, `XNAM`, `YNAM`, `ZNAM` (19, inside raw component (BFCB) bodies); signature lookups misread `VNAM` (`docs/gaps.md`). Placed only as a `REFR` base by FormID (POI design) |
| Script `VMAD` | yes | partial | partial | not-run | not-run | not-run | SCR: the script list, properties and fragments round-trip on all 12,857 official `VMAD`s through `edit::vmad`; the schema keeps `VMAD` opaque or at `reference` per type (SC). Writes: scripts attached on `set`, `new`, `place` (SCR). Fragments kept as bytes, not authored; object property values are not followed by `refs` or `formid.dangling` (STATUS "Requests", lane E -> B/D). **Scripted activator: typed-writable and corpus-verified; its in-game run (M4.O4, row O4-07) is owner-pending** |
| Conditions (`CTDA`) | yes | partial | no | not-run | not-run | not-run | `CITC`, `CIS1`, `CIS2` typed. Since `24ac42d` `CTDA`'s comparison value, parameters and reference are typed as sibling-decided unions (`common/Condition.toml`), so their FormIDs reach `refs` and `validate`; parameters 1 and 2 of the reference, actor and package functions stay as read (alias index or packdata ID by a second sibling's bit; `docs/engine-limitations.md` EL-03). No test writes a condition: copied as stored |
| `FACT` | yes | partial | yes | not-run | not-run | not-run | SC 28/0, ref 6 (`VMAD` and members of the rank group and the conditions); unknown sig `FLCS`. Used as an owner value (`PlayerFaction` in REC ownership). Writes (`24ac42d`, MX): `authoring_tests.rs` `fact_new_sets_every_typed_member_and_reads_back`; the rank group stays at `reference` |
| `GLOB` | yes | yes | yes | not-run | not-run | not-run | SC 4/0, ref 0; unknown sig `FLCS`; W1 (`new GLOB`) |
| `STAT` | yes | partial | yes | not-run | not-run | not-run | SC 21/2 (keywords in a raw component block), ref 2 (`VMAD` among them); unknown sigs `FLTR`, `INTV`, `KSIZ`, `KWDA`, `VLMS` (MX4 typed-readable `fail`, 21,408 occurrences, 56.3 %; `PTCL` typed as an opaque component since round 4); W1 and REC workshop clone it. Writes (lane D round 4): `authoring_tests.rs` `stat_new_sets_every_typed_member_and_reads_back` and `stat_dangling_link_is_reported` (MX4 typed-writable `pass`). The POI places a vanilla `STAT` (base not written), so no written `STAT` has been seen in game |
| `MSTT` | yes | partial | untested | not-run | not-run | not-run | SC 33/1, ref 8; unknown sigs `ANAM`, `BNAM`, `CNAM`, `FLCS`, `INAM`, `PTCL`, `REFL`, `SODA` |
| `DOOR` | yes | no | no | not-run | not-run | not-run | Inventory definition only: SC 6/20 (M4.BX2; `ONAM`/`CNAM` raw); unknown sigs `BNAM`, `ENAM`, `FLTR`, `INTV`, `REFL`, `STCP` |
| `CONT` | yes | partial | untested | not-run | not-run | not-run | SC 43/1, ref 8 (all nested: `MODC` in `Model`, `COED` in the item group, six `Destructible` members); no unknown sig since lane D round 4 (`DCSD`, `DCED` typed as the `DisplayCase` component layout, `PTCL` as the opaque `ParticleSystem` one; every official record decodes with nothing raw and re-encodes identically, `crates/doc/tests/authoring_types.rs`; MX4 typed-readable `pass`, 72.2 % typed bytes). **No `CONT` record has been written or seen in game**: the first POI's crate `Crate_MilitarySupply_01` (`Starfield.esm:1E4FBD`) is a `STAT` (POI) |
| `FURN` | yes | partial | untested | not-run | not-run | not-run | SC 49/2 (`NNAM` string pairs raw), ref 11; unknown sigs `ANAM`, `BNAM`, `CNAM`, `PTCL`, `REFL`, `VCCD`, `VCMT`, `VCSB`, `VCTT`, `VMRT`, `VWWD` |
| `LIGH` | yes | no | no | not-run | not-run | not-run | Inventory only: SC 5/13 (M4.BX2); unknown sigs `ANAM`, `BNAM`, `FLCS`, `FLTR`, `INTV`, `REFL` |
| `COBJ` | yes | partial | yes | not-run | not-run | not-run | SC 29/1, ref 7 (`CTDA` raw); unknown sigs `FLTR`, `INTV` (MX4 typed-readable `fail`, 154 occurrences, 84.5 %); REC workshop and fabricator (clone, `CNAM` naming a new form of the same build); writes (lane D round 4): `authoring_tests.rs` `cobj_new_sets_every_typed_member_and_reads_back` and `cobj_dangling_link_is_reported` (MX4 typed-writable `pass`); in game O4-05, O4-06 pending owner |
| `RSPJ` | yes | yes | untested | not-run | not-run | not-run | SC 17/0, ref 0 |
| `IRES` | yes | no | no | not-run | not-run | not-run | Inventory only: SC 5/9 (M4.BX2) |
| `MISC` | yes | partial | untested | not-run | not-run | not-run | SC 36/1, ref 10; unknown sigs `ANAM`, `BNAM`, `CNAM`, `REFL` |
| `FLOR` | yes | no | no | not-run | not-run | not-run | lstring-only definition: 3 of 39 seen signatures typed (PAR); unknown sigs `ANAM`, `APPR`, `BFCB`, `BFCE`, `BNAM`, `DEST`, `DSTD`, `DSTF`, `FHLS`, `FLLD`, `FLXP`, `FMAG`, `FMAH`, `FMIG`, `FMIH`, `FNAM`, `JNAM`, `KSIZ`, `KWDA`, `MODL`, `MOLM`, `OBND`, `OBTE`, `OBTF`, `OBTS`, `ODTY`, `OPDS`, `PFHS`, `PFIG`, `PFPC`, `PNAM`, `PRPS`, `PTCL`, `STOP`, `VMAD`, `XFLG` (36) |
| `AVIF` | yes | no | no | not-run | not-run | not-run | lstring-only definition: 3 of 9 seen signatures typed (PAR); unknown sigs `AVFL`, `NAM0`, `NAM1`, `NAM2`, `NAM3`, `NLDT` |
| `QUST` | yes | partial | untested | not-run | not-run | not-run | SC 178/6, ref 3 (`SNAM` SwfFile among them); unknown sig `FLLD`; stages, objectives and the three alias kinds typed at `corpus` (STATUS lane BQ); fragments, `ALFG`, `ALAM`, `ALCA` raw. A new quest is the W10 stretch, not attempted |
| `SCEN` | yes | partial | no | not-run | not-run | not-run | SC 55/1, ref 0, but 48 of 89 seen signatures unknown, action-specific subrecords kept raw (`docs/gaps.md`): `ACBS`, `AIDT`, `ALLA`, `ATAC`, `ATTR`, `ATXT`, `BIPL`, `BNAM`, `BTXT`, `CITC`, `CNAM`, `DATA`, `DMAX`, `DMIN`, `DOFT`, `DTGT`, `ESCE`, `ESCS`, `FLAV`, `FLMV`, `HTID`, `INTT`, `LCEP`, `LVCR`, `LVLO`, `MPCD`, `NVCI`, `OCOR`, `PAPI`, `PAPN`, `PASP`, `PLRL`, `PNST`, `PPST`, `QUAL`, `REPL`, `SHRT`, `SOFT`, `SPOR`, `SSPN`, `STRV`, `UNAM`, `VCLR`, `VENC`, `VNML`, `VTXT`, `WED0`, `WED1` |
| `DIAL` | yes | yes | no | not-run | not-run | not-run | SC 13/0, ref 0; unknown sigs `FLCS`, `OPDS`. Nested type (group type 10): `new` refuses it (STATUS M4.E1) |
| `INFO` | yes | partial | no | not-run | not-run | not-run | SC 56/3, ref 7 (`TPIC` among them); unknown sig `FLCS`. Nested (group type 7): refused |
| `NPC_` | yes | partial | untested | not-run | not-run | not-run | SC 114/2, ref 16; unknown sigs `FLTR`, `STCP`; every record compressed. REC actors places a vanilla `NPC_` (not written) |
| `PACK` | yes | partial | untested | not-run | not-run | not-run | SC 52/1, ref 1; data-input `CNAM`, `PTDA` target, `PLDT`/`PLVD` values, `PDTO` raw (`docs/gaps.md`) |
| `NAVM` | yes | partial | partial | not-run | not-run | not-run | SC 3/3 (`NVNM`, `MNAM` raw), ref 1; unknown sig `REFL`. Writes: import of CK navmesh only (M4.A4, `crates/esm/tests/navmesh.rs`, synthetic; F22 never made). KitKraft cannot generate navmesh |
| `PNDT` | yes | partial | untested | not-run | not-run | not-run | SC 25/0, ref 1; unknown sigs `FLLD`, `FULL`, `KSIZ`, `KWDA`, `MCQP`, `MODL`, `PCCC`, `XLMS`, `XMPM`, `XMSP` (in raw component blocks); read by `kit planet` (`crates/engine/tests/planet_corpus.rs`) |
| `PCMT` | yes | partial | untested | not-run | not-run | not-run | SC 12/0, ref 2; every tree walked (`planet_corpus.rs` `corpus_every_pcmt_tree_is_walked`) |
| `PCBN` | yes | partial | untested | not-run | not-run | not-run | SC 16/0, ref 1; unknown sigs `NAM6`..`NAM9`, `UNAM`, `VNAM`, `WNAM`, `XNAM`, `YNAM`, `ZNAM` (10, component bodies) |
| `PCCN` | yes | partial | untested | not-run | not-run | not-run | SC 8/0, ref 0; unknown sigs `CITC`, `CTDA`, `NAM1`..`NAM9`, `UNAM`, `VNAM`, `WNAM`, `XNAM`, `YNAM`, `ZNAM` (17, component bodies) |
| `LVLP` | yes | partial | untested | not-run | not-run | not-run | SC 26/1, ref 4; unknown sigs `CITC`, `NAM1`..`NAM9`, `UNAM`, `VNAM`, `WNAM`, `XNAM`, `YNAM`, `ZNAM` (16) |
| `PERK` | yes | yes | untested | not-run | not-run | not-run | SC 36/4, ref 0 |
| `FLST` | yes | yes | untested | not-run | not-run | not-run | SC 11/0, ref 0; unknown sigs `FLTR`, `INTV` |
| `LVLI` | yes | partial | untested | not-run | not-run | not-run | SC 36/1 (`COED` owner raw), ref 9; `LLCT` count not kept in step by byte-level `set` (`docs/gaps.md`) |
| `LVLN` | yes | partial | untested | not-run | not-run | not-run | SC 27/2, ref 7 |
| `CURV`, `REGN` | yes | no | no | not-run | not-run | not-run | No definition (PAR: every signature unknown) |
| `TXST`; audio (`SOUN`, `WWED`, `WKMF`) | yes | no | no | not-run | not-run | not-run | Inventory definitions only (M4.BX2) |
| `WEAP` | yes | partial | yes | not-run | pass | not-run | SC 62/2, ref 9 (`VMAD` and members of the `Destructible` group); no unknown sig since lane D round 4 (`QUPA` and `STCP` typed as the `QualityUpgrade` and `SoundTag` component layouts, `PTCL` as the opaque `ParticleSystem` one; `crates/doc/tests/authoring_types.rs`; MX4 typed-readable `pass`, 68.8 % typed bytes); REN, OVH, `set_tests.rs`. Game: G-REN (`game-20261014-01`) and G-OVH (`game-20261015-02`). O5-15 (save and reload with the weapons) not run |
| `AMDL`, `OMOD` | yes | partial (`OMOD` ref 6; `AMDL` ref 0) | yes | not-run | pass | not-run | SC AMDL 4/0, OMOD 18/2 (property values typed by sibling-field unions), OMOD ref 6 (`VMAD`, `XFLG`, `FLTR`, `NAM2`, `NAM3` among them); `OMOD` unknown sigs `KSIZ`, `KWDA`; OVH. Game: G-OVH (rows C3, C6, C7, C11 change them) |

## Capabilities

Sections A, F, G and H of the inventory, one row per capability. Record-level statuses are
in the table above; here a column is `n/a` when it does not apply.

| Capability (section) | Parsed | Typed-readable | Typed-writable | CK-diff | Game | Save/reload | Evidence and limits |
| -------------------- | ------ | -------------- | -------------- | ------- | ---- | ----------- | ------------------- |
| No-op roundtrip across record classes and group nesting (A1) | yes | n/a | n/a | not-run | n/a | n/a | RT, all 14 files, every group type |
| Unknown bodies, subrecords and component blocks kept (A1) | yes | n/a | yes | not-run | n/a | n/a | RT; PAR 0 re-encode conflicts; opaque passthrough on edit (binding constraint 4, M4) |
| Targeted edits; changed byte spans reported (A1, A3) | n/a | n/a | yes | not-run | pass | not-run | `set`/`build` write only touched records into a patch; `kit diff` (`crates/engine/tests/diff_corpus.rs`); game: REN (`game-20261014-01`) and OVH (G-OVH, `game-20261015-02`) |
| New forms, collision-safe IDs, group placement (A1) | n/a | n/a | partial | not-run | pass | not-run | M4.A3 allocator (`crates/esm/tests/forms.rs`), W1, PL; full scale only. Whether IDs match the CK's is F06 (never observed). Game: the POI's new `REFR` (G-POI) |
| References between new forms and masters (A1) | n/a | n/a | yes | not-run | not-run | not-run | `new:<name>` in spec v3 (`build_v3_tests.rs`, REC workshop); `validate` `formid.dangling` 0 errors |
| Invalid structures caught before writing (A1, H1) | yes | n/a | yes | n/a | n/a | n/a | Seeded structure-aware mutation (`crates/esm/tests/hostile.rs`, M3.A5; no coverage-guided fuzzing, `docs/gaps.md`); `validate` rules (`docs/validate.md`), among them `layout.undecoded` with the write refusal (LU); never-panic lints |
| Independent oracles (A1) | yes | yes | n/a | not-run | n/a | n/a | SF1Edit: 149,454 agree, 0 ours on the 12 round 3 types (`docs/oracles.md`, M3.B11); Papyrus compiler: PAPY (8 of 8 scripts equivalent); Spriggit is not a byte oracle (`docs/gaps.md`). CK: CK0 |
| CK golden fixtures with baseline hashes (A1, A4, H1) | n/a | n/a | n/a | not-run | n/a | n/a | CK0. `cargo xtask observe` (M3.H2) is ready to ingest them |
| Raw FormIDs beside resolved identities (A2) | yes | yes | n/a | n/a | n/a | n/a | `kitkraft-doc` resolution; `inspect` and the workbench Diagnostics panel show both (`docs/workbench.md`) |
| Missing masters fail explicitly (A2) | yes | n/a | yes | n/a | n/a | n/a | `kit load` names them (seen 2026-10-15: `SFBGS047.esm needs sfbgs004.esm`); `validate` missing-master rule |
| FormIDs re-encoded when masters change (A2) | n/a | n/a | partial | not-run | not-run | not-run | Patch remapping through typed visitors; refused when a record holds untyped or raw-union payloads (`docs/gaps.md`, "Patch FormIDs"; M3.E10 carried) |
| Referential-integrity scan (A2) | n/a | partial | n/a | n/a | n/a | n/a | `formid.dangling`, `refs` (`crates/engine/tests/refs_corpus.rs`, `validate_corpus.rs`); blind to opaque payloads and `VMAD` object values (`docs/gaps.md`; STATUS "Requests") |
| Mixed full, medium and small masters (A2, H1) | yes | yes | partial | not-run | pass, full scale only | not-run | Corpus decode with small and medium masters (`docs/load-order.md`); runtime slots inferred (`runtime-slots-by-scale`), one full slot seen in game (G-POI); no small or medium slot observed; new forms in small or medium plugins refused |
| `BlueprintShips-*` behaviour (A2) | yes | yes | n/a | not-run | not-run | not-run | Blueprint plugins take the last full slots (assumption from the owner's saves, `kit load` limitation); which version wins over a user override is open (`docs/gaps.md`) |
| Support matrix (A3) | n/a | n/a | n/a | n/a | n/a | n/a | This document and `docs/record-parity.md` "Families"; M5.D1 makes it generated |
| Provenance per member (A3) | n/a | yes | n/a | n/a | n/a | n/a | SC: every member has `source` and `confidence` |
| Base form component blocks (A3) | yes | partial | no | not-run | n/a | n/a | Since `24ac42d` four layouts are typed as optional groups led by their first subrecord (`AnimationGraph`, `ContactShadow`, `PlanetContentProperties`; `Reflection` kept raw), and since lane D round 4 four more (`DisplayCase`, `QualityUpgrade`, `SoundTag`; `ParticleSystem` kept raw as `Reflection` is: its upstream helper is in an unapproved unit) (`common/BfcbBlock.toml`, `crates/doc/tests/components.rs` over every official block); the other upstream layouts stay raw (`docs/engine-limitations.md` EL-02) |
| Schema growth leaves no-op output unchanged (A3) | yes | n/a | n/a | n/a | n/a | n/a | RT rerun after each family merge (M4.BX1 parity: 0 re-encode differences) |
| Environment manifest (A4) | n/a | n/a | n/a | n/a | n/a | n/a | `docs/compatibility.md` (M5.F3, 2026-10-16): game, CK, Papyrus compiler, Archive2, SFSE, xEdit and KitKraft versions with file hashes; every official master with size, SHA-256 (equal to `corpus/manifest.toml`), flags, `HEDR`, form version and masters; the DLC mapping checked against Steam's depots. Still no machine-readable manifest per build |
| Papyrus `.psc` -> `.pex` with the installed compiler (A4, F1) | n/a | n/a | yes | not-run | not-run | not-run | SCR (PapyrusCompiler 4.7.0.5 run as a black box); PAPY: equivalent to the compiler run by hand on 8 scripts. CK-diff stays `not-run`: no compile run by the CK has been observed |
| BA2 packaging, loose vs packed (A4, G1) | yes | n/a | yes | n/a | not-run | not-run | `ba2.pack`, `package` (`docs/packaging.md`); Archive2 opens our archives (`docs/gaps.md`); no packed plugin run in game (O2-13 `n/a`) |
| Packaging manifest (A4) | n/a | n/a | yes | n/a | n/a | n/a | `kitkraft-manifest.json` (M4.E5) |
| Isolated deploy and rollback (A4) | n/a | n/a | yes | n/a | not-run | n/a | `kit deploy --dev` (ADR 0009; `crates/engine/src/deploy/tests.rs`, synthetic MO2 only). Never run against `C:\Modding\MO2-Starfield` (STATUS, lane F) |
| Script compile, bind and fire (F1) | n/a | partial | partial | not-run | not-run | not-run | SCR. Runtime event invocation: owner-pending (M4.O4) |
| Workshop and ownership Papyrus events (F1) | n/a | yes | n/a | n/a | not-run | not-run | Declarations found by `kit papyrus events` (`docs/design/persistence.md`, Appendix A: 56 of 59 names); runtime behaviour grade D (PS claims) |
| Engine limitation register (F2) | n/a | n/a | n/a | n/a | n/a | n/a | `docs/engine-limitations.md` (M5.F1, 2026-10-16): 11 observed rows with evidence, 5 unobserved CK or game behaviours, 12 grade D hypotheses with the ledger rows that answer them |
| Vanilla mesh kits by reference (G1) | n/a | n/a | yes | not-run | not-run | not-run | Clones keep the vanilla model paths (W1, REC workshop) |
| Load-order and master audits (G2) | yes | yes | n/a | n/a | n/a | n/a | `load`, `conflicts`, `validate` |
| Agent-driven game sessions (H2, H3) | n/a | n/a | n/a | n/a | not-run | not-run | Not built. ADR 0010 approved in principle (Amendment 1); M4.T1..T10 become M5 lane C |
| Foothold gameplay regression matrix (H2) and soak (H3) | n/a | n/a | n/a | n/a | not-run | not-run | Out of KitKraft milestones (ADR 0011); generic fixtures only (FTH-EXP tickets) |

## Gate K0 (section K)

| K0 item | Satisfied | Evidence; what is missing |
| ------- | --------- | ------------------------- |
| The `WEAP` edit is a baseline regression | **yes** | REN is a workspace test (`cargo xtask ci` runs it when `KITKRAFT_DATA_DIR` is set, and skips it otherwise); game G-REN |
| Three non-weapon record-family fixtures roundtrip, one a location or reference family | **yes for KitKraft-built fixtures; no for CK-authored ones** | POI (`CELL`, `REFR`), W1 (`GLOB`, `STAT`), REC (`COBJ`, `ACHR`, `CELL` `XOWN`), SCR (`REFR` + `VMAD`): each validates 0/0 and roundtrips identical. No CK-authored fixture exists (CK0) |
| A brand-new multi-record mini-plugin with valid dependencies and no dangling references | **partly** | REC workshop: a new `STAT` and a new `COBJ` whose `CNAM` names it; `validate` 0/0 (`formid.dangling`). Two forms, data only: not run in game, and no location, terminal or keyword in it (M5.D7) |
| A scripted activator compiles, binds, loads and fires in game | **no** | Compiles and binds (SCR, `validate` 0/0). Loads and fires: owner-pending (M4.O4, O4-07). The recipe's script is on a placed `REFR` of a vanilla `ACTI` and fires on `OnLoad`, not on activation |
| The CK loads and re-exports the output, and the differential is clean | **no** | CK0: the CK has not opened a KitKraft plugin (M5.O1, then M5.A3) |

## Notes for the reader

- Every game result above rests on three owner sittings, each with a ledger entry:
  G-REN (`game-20261014-01`), G-OVH (`game-20261015-02`) and G-POI (`game-20261015-03`).
  The last two are owner reports transcribed by M5.F4 with the plugins' hashes recovered
  from the installed files, so their cells read `pass`. No row of any sitting recorded
  save and reload.
- `kit coverage --matrix` (MX) and this matrix disagree where `docs/schema/evidence.toml`
  cites no test (MX has no `TES4` row; typed-writable `COBJ`, `STAT`, `REFR`, `ACHR` and
  `CELL` agree since lane D round 4, MX4) and where the evidence file still points at
  `plan/STATUS.md` instead of the ledger entries.
  The evidence file is lane D's; M5.D1 generates this matrix from it.
- The CK's own settings files in the game folder (`CreationKitPrefs.ini`,
  `EditorWarnings.txt`) carry 2026-10-15 18:49-18:50 timestamps, so the CK may have been
  started today. No observation, fixture or ledger entry records it, so every CK-diff cell
  stays `not-run`.
- Typed-writable `untested` is not a promise: `new` and `clone` accept any typed top-level
  type and `set` any typed member, but a member below `corpus` confidence is refused unless
  allowed, and nothing has shown that the game accepts what is written.
