# Foothold gap matrix (M4-accurate)

Reconciles the owner's Foothold capability inventory (`docs/foothold/engineering-surface.md`,
sections A to I and K) with what `main` proves, as the owner's 2026-10-15 decisions ask
(priority F, "Deliverable"). Established on `main` at `fcb443a` on 2026-10-15 from the
tests in `crates/*/tests` and the handler tests, `kit schema` and `kit coverage` (release
build of that commit), `docs/record-parity.md`, the two ledgers and `plan/STATUS.md`.
Nothing here is a plan: the tickets that close the gaps are M5 in `plan/execution-plan.md`.
Update a cell only with the commit, test or ledger entry that changes it.

## Statuses

Six statuses, never collapsed into one (decisions, "Acceptance criteria"; AGENTS.md
"Reporting"). Schema presence is not feature support.

| Column | Meaning | Values |
| ------ | ------- | ------ |
| Parsed | Every corpus record of the type is read and written back byte-identically (unedited) | yes, no |
| Typed-readable | The schema types its members at `corpus` confidence and every corpus occurrence decodes, with 0 re-encode conflicts | yes; partial (a payload the feature needs is opaque or unknown); no (inventory or lstring-only definition, or none) |
| Typed-writable | A KitKraft write of the type (`set`, `new`, `clone`, `place`, `build`) is covered by a test or a built, validated, roundtripped recipe | yes; partial; untested (the generic path accepts it, nothing proves it); no (refused, or members opaque) |
| CK-diff | A CK-authored or CK-re-exported fixture compared against KitKraft (`docs/ck-observations.md`) | pass, fail, not-run, inconclusive |
| Game | Observed in the running game on `KitKraft Dev` (`docs/game-observations.md`, or an owner result in `plan/STATUS.md`) | pass, fail, not-run, inconclusive |
| Save/reload | The game observation survives save, quit and reload (persistence protocol steps 1 to 4) | pass, fail, not-run, inconclusive |

`n/a`: the status does not apply to that row.

## Evidence keys

| Key | Evidence |
| --- | -------- |
| RT | Corpus roundtrip: all 14 official files byte-identical (`crates/esm/tests/corpus.rs` `corpus_full_roundtrip`; STATUS "Done", M1.6); compressed records and `XXXX` (`corpus_small_compressed_and_xxxx`); 136,741 compressed records decoded, 0 failures (`docs/record-parity.md`) |
| SC | `kit schema` at `fcb443a`: 96 definitions, 2,209 members (2,005 typed, 204 opaque), confidence `corpus` 1,957, `reference` 250, `container-reference` 2, **`ck-diff` 0, `verified` 0**. Per type "typed/opaque, ref N" below quotes it; "unknown sigs" quotes the corpus signatures the definition does not name (`docs/record-parity.md`) |
| PAR | `docs/record-parity.md` (generated): 0 decode failures and 0 re-encode conflicts over the corpus; `kit coverage SFBGS00D.esm` on 2026-10-15: 434,991 records, 0 failed, 0 re-encode conflicts, decoded by confidence `ck-diff` 0, `verified` 0 |
| W1 | `crates/engine/src/handlers/newform_tests.rs`; `out\w1\KitKraftW1.esm` (`new GLOB`, `clone` of STAT `Starfield.esm:1E4FBD`; SHA-256 `5d122dd3...`; STATUS "Done", M4.E1) |
| PL | `crates/engine/src/handlers/place_tests.rs`; `crates/engine/tests/place_corpus.rs` (1,000 corpus references recomposed byte-identically); `crates/esm/tests/rebuild.rs` (M4.A2 rebuild gate) |
| POI | `docs/poi/first-poi.md`: `KitKraftFirstPOI.esm` (SHA-256 `13c22b48...`), one persistent `REFR` of a vanilla STAT in the carried cell `CityNewAtlantisApartmentTEMPLATE`; validate 0/0, roundtrip identical |
| OVH | `crates/engine/tests/overhaul_build.rs`; `docs/overhaul/build-2.md` (21 overrides: WEAP 13, AMDL 4, OMOD 4) |
| REN | `crates/engine/tests/smoke_rename.rs` (WEAP `FULL` rename) |
| REC | Recipes built, validated 0/0 and roundtripped identical (STATUS "Done", lane E round 5): `docs/recipes/workshop.md` (clone STAT + clone COBJ with `CNAM = "new:coffee"`), `fabricator.md` (clone COBJ), `ownership.md` (`CELL` `XOWN`), `actors.md` (`ACHR`); test `field_values_name_new_forms_of_the_build` |
| SCR | `docs/recipes/scripts.md`: `crates/engine/tests/vmad_corpus.rs` (release, `#[ignore]`d by default: all 12,857 official `VMAD`s re-encode identically; attaching keeps vanilla scripts and fragments); `crates/engine/src/handlers/scripts_tests.rs` (`set`, `new`, `place` attach scripts; the two papyrus rules); `KitKraftScripts.esm` (SHA-256 `b20cf577...`) and `HelloOnLoad.pex` compiled by PapyrusCompiler 4.7.0.5 |
| G-REN | `docs/game-observations.md` entry `game-20261014-01` (rename, O2-01 **pass**) |
| G-OVH | `plan/STATUS.md` "M3.O5 result (2026-10-15): PASS" (owner; C1-C7, C9-C14). **No ledger entry** |
| G-POI | `plan/STATUS.md` "M4.O2 result (2026-10-15): PASS" (owner reached the crate with `player.moveto 04001000`; commit `a5f08c9`). **No ledger entry**; the save/quit/reload row is "pending owner confirmation" |
| CK0 | `docs/ck-observations.md` "Ledger": "No entries yet". No CK fixture exists in the repo or `observe\`. M3.O1 and M4.O1 were deferred by the owner (STATUS "Decision records") |

Every row's CK-diff is `not-run` (CK0, and SC/PAR show no member at `ck-diff`). Save/reload
is `not-run` everywhere: no session has recorded the persistence protocol.

## Record families

One row per record type the inventory names (sections B to G and the matrix in section
I), plus the M3 baseline types.

| Type | Parsed | Typed-readable | Typed-writable | CK-diff | Game | Save/reload | Evidence and limits |
| ---- | ------ | -------------- | -------------- | ------- | ---- | ----------- | ------------------- |
| `TES4` | yes | yes | yes | not-run | pass | not-run | RT; SC 8/3, ref 2 (container-reference); writer keeps `HEDR` count and next object ID, `INCC`, `ONAM` by the corpus rules (`crates/engine/tests/writing_rules.rs`, PL). Game: every game-tested plugin's header was written by KitKraft (G-REN, G-POI). Limits: header stamps' meaning unknown (POI); `ONAM` order is the writer's (`docs/gaps.md`) |
| `GRUP`, compressed records, `XXXX` | yes | n/a | yes | not-run | pass | not-run | RT; computed cell and world paths pass the rebuild gate (PL); edited compressed records recompress identically for 99.88% (`docs/gaps.md`, "Recompression parity"). Game: the POI's group path (G-POI). Limits: what the CK writes for a new plugin's groups is not observed (`docs/gaps.md`) |
| Master flags and scales | yes | n/a | partial | not-run | pass | not-run | Full, small, medium and blueprint flags read as independent dimensions (`docs/load-order.md`; corpus has small `SFBGS004`, `SFBGS047`, medium `SFBGS003`, `SFBGS006`, blueprint `BlueprintShips-*`). Writes: full scale only; new forms in small or medium plugins refused (`place_tests.rs` `a_small_master_refuses_new_forms`). Game: one full slot (`04001000`) seen (G-POI); no `FE`/`FD` ID checked (`docs/gaps.md`) |
| `LCTN` | yes | yes | untested | not-run | not-run | not-run | SC 38/2 (VMAD, NAM0), ref 8 (removed and added persistent refs, world cells); PAR 29 of 29 seen signatures typed; read by `kit planet` and `refs`. No test or recipe writes one |
| `CELL` | yes | yes | partial | not-run | pass | not-run | SC 33/2, ref 4; `unordered` mode (STATUS M4.BX1). Writes: carried copy of a vanilla cell (PL, POI) and `XOWN` override (REC ownership, not run in game). New cells refused (`new`/`place`; STATUS M4.E1). Game: the carried apartment cell (G-POI); save/reload row pending owner |
| `WRLD` | yes | yes | partial | not-run | not-run | not-run | SC 38/2, ref 7; 9 unknown sigs. Writes: carried copy for an exterior placement (`place_tests.rs` `world_positions_pick_the_exterior_or_the_persistent_cell`); new worldspaces refused. No exterior plugin run in game |
| `REFR` | yes | yes | yes | not-run | pass | not-run | SC 93/7, ref 5; PL, POI. Game: the POI crate (G-POI). Limits: flag 0x800 (initially disabled) only with `allow_reference` (`docs/gaps.md`); XSL1, XNSE, XTV2 raw |
| `ACHR` | yes | yes | yes | not-run | not-run | not-run | SC 42/3, ref 11; PL (`ACHR` for an `NPC_` base), REC actors (O4-03 pending owner) |
| `ACTI` | yes | yes | untested | not-run | not-run | not-run | SC 45/2, ref 9. The scripts recipe places a `REFR` of the vanilla `ACTI` `ActivatorSwitch01` and writes `VMAD` on the reference; no `ACTI` record is written |
| `KYWD` | yes | yes | untested | not-run | not-run | not-run | SC 9/2 (FNAM opaque, VMAD at reference). Read as the outpost and workbench categories (REC workshop, fabricator) |
| `TERM` | yes | partial | untested | not-run | not-run | not-run | SC 57/2, ref 19 (conditions, `NTRM` menu, sounds); `WBDT` and `VMAD` opaque (`docs/gaps.md`, items family). `TMLM` (menus) SC 24/1 |
| `PKIN` | yes | partial | untested | not-run | not-run | not-run | SC 22/1, all `corpus`; 19 unknown sigs inside raw component (BFCB) bodies; signature lookups misread `VNAM` (`docs/gaps.md`). Placed only as a `REFR` base by FormID (POI design) |
| Script `VMAD` | yes | partial | partial | not-run | not-run | not-run | SCR: the script list, properties and fragments round-trip on all 12,857 official `VMAD`s through `edit::vmad`; the schema keeps `VMAD` opaque or at `reference` per type (SC). Writes: scripts attached on `set`, `new`, `place` (SCR). Fragments kept as bytes, not authored; object property values are not followed by `refs` or `formid.dangling` (STATUS "Requests", lane E -> B/D). **Scripted activator: typed-writable and corpus-verified; its in-game run (M4.O4, row O4-07) is owner-pending** |
| Conditions (`CTDA`) | yes | partial | no | not-run | not-run | not-run | `CITC`, `CIS1`, `CIS2` typed; `CTDA` kept raw on every type (a flag- and function-decided union, `docs/gaps.md`); copied as bytes only |
| `FACT` | yes | yes | untested | not-run | not-run | not-run | SC 28/0, ref 6 (ranks, conditions, `VMAD`). Used as an owner value (`PlayerFaction` in REC ownership); no `FACT` record written |
| `GLOB` | yes | yes | yes | not-run | not-run | not-run | SC 4/0; W1 (`new GLOB`) |
| `STAT` | yes | yes | yes | not-run | not-run | not-run | SC 21/2 (keywords in a raw component block); W1 and REC workshop clone it. The POI places a vanilla `STAT` (base not written), so no written `STAT` has been seen in game |
| `MSTT` | yes | yes | untested | not-run | not-run | not-run | SC 33/1, ref 8 |
| `DOOR` | yes | no | no | not-run | not-run | not-run | Inventory definition only: SC 6/20 (M4.BX2; `ONAM`/`CNAM` raw) |
| `CONT` | yes | yes | untested | not-run | not-run | not-run | SC 43/1, ref 8. **No `CONT` record has been written or seen in game**: the first POI's crate `Crate_MilitarySupply_01` (`Starfield.esm:1E4FBD`) is a `STAT` (POI) |
| `FURN` | yes | yes | untested | not-run | not-run | not-run | SC 49/2 (`NNAM` string pairs raw), ref 11 |
| `LIGH` | yes | no | no | not-run | not-run | not-run | Inventory only: SC 5/13 (M4.BX2) |
| `COBJ` | yes | yes | yes | not-run | not-run | not-run | SC 29/1, ref 7 (`CTDA` raw); REC workshop and fabricator (clone, `CNAM` naming a new form of the same build); in game O4-05, O4-06 pending owner |
| `RSPJ` | yes | yes | untested | not-run | not-run | not-run | SC 17/0 |
| `IRES` | yes | no | no | not-run | not-run | not-run | Inventory only: SC 5/9 (M4.BX2) |
| `MISC` | yes | yes | untested | not-run | not-run | not-run | SC 36/1, ref 10 |
| `FLOR` | yes | no | no | not-run | not-run | not-run | lstring-only definition: 3 of 39 seen signatures typed (PAR) |
| `AVIF` | yes | no | no | not-run | not-run | not-run | lstring-only definition: 3 of 9 seen signatures typed (PAR) |
| `QUST` | yes | yes | untested | not-run | not-run | not-run | SC 178/6, ref 3; stages, objectives and the three alias kinds typed at `corpus` (STATUS lane BQ); fragments, `ALFG`, `ALAM`, `ALCA` raw. A new quest is the W10 stretch, not attempted |
| `SCEN` | yes | partial | no | not-run | not-run | not-run | SC 55/1, but 48 of 89 seen signatures unknown: action-specific subrecords kept raw (`docs/gaps.md`) |
| `DIAL` | yes | yes | no | not-run | not-run | not-run | SC 13/0. Nested type (group type 10): `new` refuses it (STATUS M4.E1) |
| `INFO` | yes | yes | no | not-run | not-run | not-run | SC 56/3, ref 7. Nested (group type 7): refused |
| `NPC_` | yes | yes | untested | not-run | not-run | not-run | SC 114/2, ref 16; every record compressed. REC actors places a vanilla `NPC_` (not written) |
| `PACK` | yes | partial | untested | not-run | not-run | not-run | SC 52/1; data-input `CNAM`, `PTDA` target, `PLDT`/`PLVD` values, `PDTO` raw (`docs/gaps.md`) |
| `NAVM` | yes | partial | partial | not-run | not-run | not-run | SC 3/3 (`NVNM`, `MNAM` raw). Writes: import of CK navmesh only (M4.A4, `crates/esm/tests/navmesh.rs`, synthetic; F22 never made). KitKraft cannot generate navmesh |
| `PNDT` | yes | partial | untested | not-run | not-run | not-run | SC 25/0, ref 1; 10 unknown sigs in raw component blocks; read by `kit planet` (`crates/engine/tests/planet_corpus.rs`) |
| `PCMT` | yes | yes | untested | not-run | not-run | not-run | SC 12/0, ref 2; every tree walked (`planet_corpus.rs` `corpus_every_pcmt_tree_is_walked`) |
| `PCBN` | yes | partial | untested | not-run | not-run | not-run | SC 16/0; 10 unknown sigs (component bodies) |
| `PCCN` | yes | partial | untested | not-run | not-run | not-run | SC 8/0; 17 unknown sigs (component bodies) |
| `LVLP` | yes | partial | untested | not-run | not-run | not-run | SC 26/1, ref 4; 16 unknown sigs |
| `PERK` | yes | yes | untested | not-run | not-run | not-run | SC 36/4 |
| `FLST` | yes | yes | untested | not-run | not-run | not-run | SC 11/0 |
| `LVLI` | yes | yes | untested | not-run | not-run | not-run | SC 36/1 (`COED` owner raw); `LLCT` count not kept in step by byte-level `set` (`docs/gaps.md`) |
| `LVLN` | yes | yes | untested | not-run | not-run | not-run | SC 27/2 |
| `CURV`, `REGN` | yes | no | no | not-run | not-run | not-run | No definition (PAR: every signature unknown) |
| `TXST`; audio (`SOUN`, `WWED`, `WKMF`) | yes | no | no | not-run | not-run | not-run | Inventory definitions only (M4.BX2) |
| `WEAP` | yes | yes | yes | not-run | pass | not-run | SC 62/2, ref 9; REN, OVH, `set_tests.rs`. Game: G-REN, G-OVH. O5-15 (save and reload with the weapons) not recorded |
| `AMDL`, `OMOD` | yes | yes | yes | not-run | pass | not-run | SC AMDL 4/0, OMOD 18/2 (property values typed by sibling-field unions); OVH. Game: G-OVH (rows C3, C6, C7, C11 change them) |

## Capabilities

Sections A, F, G and H of the inventory, one row per capability. Record-level statuses are
in the table above; here a column is `n/a` when it does not apply.

| Capability (section) | Parsed | Typed-readable | Typed-writable | CK-diff | Game | Save/reload | Evidence and limits |
| -------------------- | ------ | -------------- | -------------- | ------- | ---- | ----------- | ------------------- |
| No-op roundtrip across record classes and group nesting (A1) | yes | n/a | n/a | not-run | n/a | n/a | RT, all 14 files, every group type |
| Unknown bodies, subrecords and component blocks kept (A1) | yes | n/a | yes | not-run | n/a | n/a | RT; PAR 0 re-encode conflicts; opaque passthrough on edit (binding constraint 4, M4) |
| Targeted edits; changed byte spans reported (A1, A3) | n/a | n/a | yes | not-run | pass | not-run | `set`/`build` write only touched records into a patch; `kit diff` (`crates/engine/tests/diff_corpus.rs`); game: REN, OVH |
| New forms, collision-safe IDs, group placement (A1) | n/a | n/a | partial | not-run | pass | not-run | M4.A3 allocator (`crates/esm/tests/forms.rs`), W1, PL; full scale only. Whether IDs match the CK's is F06 (never observed). Game: the POI's new `REFR` (G-POI) |
| References between new forms and masters (A1) | n/a | n/a | yes | not-run | not-run | not-run | `new:<name>` in spec v3 (`build_v3_tests.rs`, REC workshop); `validate` `formid.dangling` 0 errors |
| Invalid structures caught before writing (A1, H1) | yes | n/a | yes | n/a | n/a | n/a | Seeded structure-aware mutation (`crates/esm/tests/hostile.rs`, M3.A5; no coverage-guided fuzzing, `docs/gaps.md`); `validate` rules (`docs/validate.md`); never-panic lints |
| Independent oracles (A1) | yes | yes | n/a | not-run | n/a | n/a | SF1Edit: 149,454 agree, 0 ours on the 12 round 3 types (`docs/oracles.md`, M3.B11); Spriggit is not a byte oracle (`docs/gaps.md`). CK: CK0 |
| CK golden fixtures with baseline hashes (A1, A4, H1) | n/a | n/a | n/a | not-run | n/a | n/a | CK0. `cargo xtask observe` (M3.H2) is ready to ingest them |
| Raw FormIDs beside resolved identities (A2) | yes | yes | n/a | n/a | n/a | n/a | `kitkraft-doc` resolution; `inspect` and the workbench Diagnostics panel show both (`docs/workbench.md`) |
| Missing masters fail explicitly (A2) | yes | n/a | yes | n/a | n/a | n/a | `kit load` names them (seen 2026-10-15: `SFBGS047.esm needs sfbgs004.esm`); `validate` missing-master rule |
| FormIDs re-encoded when masters change (A2) | n/a | n/a | partial | not-run | not-run | not-run | Patch remapping through typed visitors; refused when a record holds untyped or raw-union payloads (`docs/gaps.md`, "Patch FormIDs"; M3.E10 carried) |
| Referential-integrity scan (A2) | n/a | partial | n/a | n/a | n/a | n/a | `formid.dangling`, `refs` (`crates/engine/tests/refs_corpus.rs`, `validate_corpus.rs`); blind to opaque payloads and `VMAD` object values (`docs/gaps.md`; STATUS "Requests") |
| Mixed full, medium and small masters (A2, H1) | yes | yes | partial | not-run | partial | not-run | Corpus decode with small and medium masters (`docs/load-order.md`); runtime slots inferred (`runtime-slots-by-scale`), one full slot seen in game (G-POI); new forms in small or medium plugins refused |
| `BlueprintShips-*` behaviour (A2) | yes | yes | n/a | not-run | not-run | not-run | Blueprint plugins take the last full slots (assumption from the owner's saves, `kit load` limitation); which version wins over a user override is open (`docs/gaps.md`) |
| Support matrix (A3) | n/a | n/a | n/a | n/a | n/a | n/a | This document and `docs/record-parity.md` "Families"; M5.D1 makes it generated |
| Provenance per member (A3) | n/a | yes | n/a | n/a | n/a | n/a | SC: every member has `source` and `confidence` |
| Base form component blocks (A3) | yes | no | no | not-run | n/a | n/a | Kept raw on every type (a component-name decider is missing, `docs/gaps.md`) |
| Schema growth leaves no-op output unchanged (A3) | yes | n/a | n/a | n/a | n/a | n/a | RT rerun after each family merge (M4.BX1 parity: 0 re-encode differences) |
| Environment manifest (A4) | n/a | n/a | n/a | n/a | n/a | n/a | Partial: `corpus/manifest.toml` (master SHA-256), `kit doctor` (game 1.16.244.0), package manifest (KitKraft version), xEdit pin (`docs/xedit-reference.md`). No single manifest; CK build recorded only in `docs/source-registry.md` (1.16.244.0). M5.F3 |
| Papyrus `.psc` -> `.pex` with the installed compiler (A4, F1) | n/a | n/a | yes | not-run | not-run | not-run | SCR (PapyrusCompiler 4.7.0.5 run as a black box). Not compared with a CK-run compile (M5.B2) |
| BA2 packaging, loose vs packed (A4, G1) | yes | n/a | yes | n/a | not-run | not-run | `ba2.pack`, `package` (`docs/packaging.md`); Archive2 opens our archives (`docs/gaps.md`); no packed plugin run in game (O2-13 `n/a`) |
| Packaging manifest (A4) | n/a | n/a | yes | n/a | n/a | n/a | `kitkraft-manifest.json` (M4.E5) |
| Isolated deploy and rollback (A4) | n/a | n/a | yes | n/a | not-run | n/a | `kit deploy --dev` (ADR 0009; `crates/engine/src/deploy/tests.rs`, synthetic MO2 only). Never run against `C:\Modding\MO2-Starfield` (STATUS, lane F) |
| Script compile, bind and fire (F1) | n/a | partial | partial | not-run | not-run | not-run | SCR. Runtime event invocation: owner-pending (M4.O4) |
| Workshop and ownership Papyrus events (F1) | n/a | yes | n/a | n/a | not-run | not-run | Declarations found by `kit papyrus events` (`docs/design/persistence.md`, Appendix A: 56 of 59 names); runtime behaviour grade D (PS claims) |
| Engine limitation register (F2) | n/a | n/a | n/a | n/a | n/a | n/a | None yet; "Not in M4" and `docs/gaps.md` hold the pieces. M5.F1 |
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
| The CK loads and re-exports the output, and the differential is clean | **no** | CK0: the CK has not opened a KitKraft plugin (M5.A1, M5.O1) |

## Notes for the reader

- Every game `pass` above rests on three owner sittings; two of them (G-OVH, G-POI) are in
  `plan/STATUS.md` only. Under the ruling's evidence rule they should be in the ledger with
  their hashes: M5.F4 transcribes them.
- The CK's own settings files in the game folder (`CreationKitPrefs.ini`,
  `EditorWarnings.txt`) carry 2026-10-15 18:49-18:50 timestamps, so the CK may have been
  started today. No observation, fixture or ledger entry records it, so every CK-diff cell
  stays `not-run`.
- Typed-writable `untested` is not a promise: `new` and `clone` accept any typed top-level
  type and `set` any typed member, but a member below `corpus` confidence is refused unless
  allowed, and nothing has shown that the game accepts what is written.
