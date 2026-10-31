# Tickets for the local agent (ADR 0014)

Each ticket is one file, `<ID>.toml`, run by the overnight harness as one unattended
supervised-solo run (`docs/local-agent.md`). The agent sees it rendered as `TICKET.md`.

## Format

```toml
id = "KK-001"                 # also the branch name: ticket/KK-001
title = "One line"
timeout_min = 45              # per grading command (default 45)
gate = "cargo xtask ci"       # default; the Linux gate
after = ["KK-006"]            # optional: queued only once these have merged to main
hold = "why"                  # optional: written, but never queued automatically
budget_h = 1.5                # optional: the run's time cap (default 1.5 h, then overtime)
priority = 1                  # optional: auto picks higher first (default 0, then ID order)
base = "kk/KK-002-wip"        # optional: continue an earlier run's branch (default main)
owns = [                      # the only paths the agent may change (prefixes or globs);
  "crates/engine/src/handlers/",  # plan/STATUS.md's one "Landed" line is always allowed
]
brief = """
What to do and why, in a few paragraphs: the files to start from, the rules that matter,
what "done" means beyond the commands, and what not to touch.
"""

[[accept]]                    # every command must exit 0; run as `nobody`, offline, in a copy
name = "engine-lib"
cmd = "cargo test -p kitkraft-engine --lib"
```

## Writing a good ticket

The director A/B (ADR 0014, "Context") is the guide:

- **Vertical and small.** One behaviour, end to end, that a 27B model can finish in one to three
  hours of Rust compiles. Split anything that touches more than one crate's public API.
- **Acceptance commands decide.** Write them before the run, make them fail on `main`, and make
  them headless: no window, no game, no Creation Kit, no network.
- **Name the starting files.** The agent must not read `plan/STATUS.md` or
  `plan/execution-plan.md`, so the brief carries the context those would give.
- **Owned paths are enforced.** A change outside `owns` fails the `scope` check.

- **Acceptance commands must not pass vacuously.** A `cargo test` filter that matches no test
  exits 0. Name the test with `--exact` and check the count:
  `... 2>&1 | tee /tmp/o; grep -q '^test result: ok. 1 passed' /tmp/o`. A corpus test that
  skips without `KITKRAFT_DATA_DIR` also passes, so have it print a count and grep for that.
- **Brief and acceptance agree** (KK-002 failed three nights on this): if the brief lets a test
  be `#[ignore]`d, the acceptance accepts `1 ignored` for it; if the acceptance requires a test
  to pass, the brief offers no way out.

`python3 plan/tickets/lint.py [--min N] [--all] FILE_OR_DIR...` (WSL or Linux: Python 3.11+)
checks these rules and the format: required fields, `after` and `base`, owned paths that
exist, acceptance that can fail and cannot pass vacuously, test names that are new and that
the agent may write, repo paths the brief names, and the KK-002 rule. It exits 1 on an error;
warnings are for a reader to judge. Merged tickets are skipped unless `--all`. Run it on a
ticket before queueing it.

## Drafts

The local agent can write tickets too. A drafting ticket (KK-050 to KK-052) asks for named
drafts, `plan/tickets/drafts/<slug>.toml`, and owns exactly those files (so drafting tickets
don't overlap and can run the same night; its gate is the lint on the folder), each a
complete ticket with `id = "DRAFT"` (the ID is assigned at promotion); its acceptance is the
lint on those files with `--min N` plus a check that each covers its topic. Drafts are never
queued: the backlog reads only `plan/tickets/KK-*.toml`.

The morning merge promotes them (`kk-merge-prompt.md`, "Drafts"): it reviews each draft as
it would a ticket (scope, acceptance that fails on main and cannot pass vacuously, a brief
that carries the context, size), runs the lint and `kk_backlog.py preflight <path>` on it,
moves the good ones to `plan/tickets/<next free ID>.toml` with a row in the table below
(source "drafted by the local agent (KK-05x)"), fixing small things in passing, and deletes
the rest with a one-line reason in its report. `plan/tickets/drafts/` is empty on `main`
after each merge; a rejected draft stays readable in the merge commit.

## Generated tickets

`python3 plan/tickets/generate.py inventory|depth [--dry-run] [--limit N]` (WSL or Linux)
turns the record-coverage gaps into tickets with no model in the loop. `inventory` (level 1,
KK-100..) batches the record types the corpus shows with no definition, most records first,
five to a ticket (up to eight small ones), in KK-022's shape, with each type's record count,
name and xEdit line range in the brief; `depth` (level 2, KK-200..) gives each defined type
with five or more opaque or unknown signatures a typing ticket in KK-023's shape, with a
parity target (typed at least half of what a transcription can type, no new unknown). Both
skip types a ticket or draft already covers, and print what they skipped and why (covered,
not in the pinned xEdit file, component-block bodies the format keeps raw, hard cases such as
SCEN and REFR). Rerun after merges and after `cargo xtask parity`: it is idempotent (a
generated ticket owns its definition files, so its types count as covered), never rewrites a
ticket, and adds README rows only for new IDs. Lint and preflight new tickets as any other.

Schema tickets on disjoint types may run the same night. What they share, and how a merge
resolves it:

- Regenerated, never merged by hand: `crates/esm/src/generated/records.rs` (`cargo xtask
  codegen`), `crates/schema/tests/golden/seed.rs` (`KITKRAFT_UPDATE_GOLDEN=1 cargo test -p
  kitkraft-schema --test codegen`) and `docs/record-parity.md` (`cargo xtask parity`, with
  `KITKRAFT_DATA_DIR`; merge any `xtask/src/parity.rs` change first). Take either side, merge
  the definitions, rerun the three.
- Sorted lists, merged by keeping both sides' entries: `BUILTIN_DEFINITIONS` in
  `crates/schema/src/lib.rs` (one entry per line; `cargo test -p kitkraft-schema` prints any
  missing line), `TYPES` in `crates/esm/tests/inventory_types.rs`, and in
  `crates/schema/tests/defs.rs` the record and common lists of
  `builtin_set_loads_and_matches_the_directory` and `LSTRING_ONLY`/`BX2_INVENTORY` (a typing
  ticket removes its type). Until KK-090 lands rustfmt packs the last three, so any two
  tickets conflict there; KK-090 derives the record lists from the directory and puts the
  others one per line, after which git merges most pairs cleanly.
- Hand-written prose, merged by hand (sections and rows): `docs/schema/*-family.md`,
  `docs/gaps.md`.

Then `cargo test -p kitkraft-schema`, `cargo xtask codegen --check` and the gate.

## Queue

`overnight kitkraft auto 8` runs every runnable ticket back to back in one budget
(`docs/local-agent.md`): not merged (main has no "Merge ticket/<ID>:" commit), not held, and
every ticket in its `after` merged. It skips a ticket whose `owns` overlaps one already picked
(each starts from `main` and they would conflict), except a shared Markdown file, merged by
hand like the STATUS line. `overnight kitkraft KK-002+KK-003 8` runs a list instead.

Before the first run, a preflight drops any ticket whose base doesn't resolve, whose owned
paths neither exist nor have an existing parent, or whose acceptance commands all pass on the
base already (one that passes is a warning: it may be vacuous). Each ticket gets up to its
`budget_h`; when every ticket has had its turn, unfinished ones get an overtime run that
continues their branch. When the queue ends, the morning merge (an Opus session) reviews each
run, merges what holds up, runs the Windows gate, pushes `main`, and updates this table.

| Ticket | Title | Source | State |
| ------ | ----- | ------ | ----- |
| [KK-001](KK-001.toml) | The gate passes on Linux | the 2026-10-23 trial: 20 tests assume Windows | merged `34e4a55` |
| [KK-002](KK-002.toml) | Typed-write tests for ACTI, KYWD, TMLM, QUST and WRLD | next-sprint item 2 (evidence entries) | merged `83ce824` (fourth run, squashed with its WIP commits; review: the QUST layout comment says its theory is unconfirmed). ACTI, KYWD, TMLM typed-writable; QUST read-back and both WRLD tests ignored with reasons; follow-ups KK-220 (QUST slots) and KK-221 (`new` for WRLD) |
| [KK-003](KK-003.toml) | MCP progress notifications | next-sprint item 6 (M6.P2b rest) | merged `472871e` |
| [KK-004](KK-004.toml) | Clone-sample corpus tests for LCTN and TERM | next-sprint item 2 (M5.D2, M5.D3 rest) | merged `1f0940c` |
| [KK-005](KK-005.toml) | validate warns when a location's added persistent reference is temporary | M5.D2 rest | merged `f4acf8e` |
| [KK-006](KK-006.toml) | Type ARMA and name the biped slot bits | apparel requests 1 and 2 | merged `e7bc2d2` |
| [KK-007](KK-007.toml) | Decode REFL losslessly; LMSW swap entries in `inspect --full` | apparel request 7 | merged `7313fbf` |
| [KK-008](KK-008.toml) | Inventory definition for LMSW, the layered material swap | apparel request 7 (`everyday-outfits`) | merged `479cdfa` (parity's Record types row still counts the 4 BFCB-internal signatures as unknown; the ticket's grep matched the Decoded row) |
| [KK-010](KK-010.toml) | testplan: add, equip and look at new and overridden armor | apparel request 5 | merged `33307be` |
| [KK-011](KK-011.toml) | Corpus survey: which object mod properties armor carries | apparel request 3 (second half) | merged `37712ee` (review: the null and unresolved counts cover only the armor union; distinct examples) |
| [KK-012](KK-012.toml) | Recipe: a location of your own (`docs/recipes/location.md`) | M5.D2 rest | merged `993b766` |
| [KK-013](KK-013.toml) | Recipe: a terminal with its own menus (`docs/recipes/terminal.md`) | M5.D3 rest | merged `3c66df6` (review: the 16 conditions pinned by count; doc fixes) |
| [KK-014](KK-014.toml) | Clone-sample corpus tests for PKIN and FACT | M5.D4, M5.D5 rest | merged `a236ffd` |
| [KK-015](KK-015.toml) | Ownership recipe, new-faction variant | M5.D5 rest | merged `0cdcb08` |
| [KK-016](KK-016.toml) | `cargo xtask parity` generates the gap matrix headline and checks the family table | M5.D1 rest | merged `d689876` (record-parity.md regenerated in the merge) |
| [KK-017](KK-017.toml) | Typed-write tests for CONT, MISC, FLST and LVLI | gap matrix "untested" rows | merged `14800e4`, review `c921095` (the gap-matrix family table's CONT, MISC, FLST cells follow MX). LVLI read-back ignored with its reason: `new` writes the entries' conditions one slot early (BFCB block condition slot), an engine fix |
| [KK-018](KK-018.toml) | validate warns when a model file a record names is nowhere to be found | apparel requests 4 and 8 (asset checks) | merged `a1d1af6` (review: model fixtures planted in two handler tests outside `owns`, which the new warning broke) |
| [KK-019](KK-019.toml) | Write LMSW material swaps: `swap_overrides` on `set` and `clone` | apparel request 7 (write half) | merged `9682af7` (review: an empty `swap_overrides` array is refused; the corpus self-rewrite skips keys named by two entries of one payload, 124 payloads, since a duplicate key is refused by design) |
| [KK-020](KK-020.toml) | Recipe: a clothing colourway (build spec `swap_overrides`) | apparel request 7 | merged `494efe1` |
| [KK-021](KK-021.toml) | Typed-write tests for ARMO, ARMA and OTFT | apparel requests 1 and 2 | merged `496587e` |
| [KK-022](KK-022.toml) | Inventory definitions for CURV, REGN, AVMD, IPCT and DFOB | record parity (types with no definition) | merged `8b00cae` (6/7: scope failed only because the run merged main into its old WIP base, so the diff from `kk/KK-022-wip` named main's own files; from main it touches only the owns and its STATUS line. Rewritten with `inventory-def`, `inventory-check` passes for all five; the merge reran `add-def` for main's reconciled parity generator) |
| [KK-023](KK-023.toml) | Type FLOR (flora) from the pinned xEdit file | record parity (lstring-only FLOR) | merged `5a320f8` (overtime run, 5/6: scope failed only on `docs/foothold/gap-matrix.md`, parity output outside the owns, as KK-201; squashed with the harness WIP commit; FLOR 37 typed, 2 opaque, 0 unknown; `flora_types_full` 302/302) |
| [KK-030](KK-030.toml) | Engine test support: game-session fakes that front ends can drive | M6.G4 to G6 against M4.T3's fakes (next-sprint item 3) | merged `5bdb830` |
| [KK-031](KK-031.toml) | The workbench gate permits `game.session` (start) and `game.stop`; `game.status` allowed | M6.G4, first half | merged `b28ea17` (note: the source scan's `Session` needle is now whole-word so `SessionForm` passes) |
| [KK-032](KK-032.toml) | The Cockpit launches a game session and follows it by polling `game.status` | M6.G4, second half | merged `9b81fd7` (QA: CHANGES, fixed in review: the start spends its dry run, so Launch and its confirm stay closed while a session is followed and a refused second start no longer ends the following; harness WIP folded in) |
| [KK-033](KK-033.toml) | The Cockpit shows a session's probe rows and screenshots | M6.G5, first half | merged `5ef2df9` (review `37b1e61`: a screenshot marked not listed or missing on disk shows that reason and is not opened, so no stale read error is cached) |
| [KK-034](KK-034.toml) | `game.probe` from the Cockpit: re-run, screenshot, send a row to the Ledger | M6.G5 second half, M6.G6 wiring | merged `3affbc8` (5/5, QA ACCEPT on the second pass; a check-in's `ticket_defect` on an engine `mod_from_brief` gate failure did not recur: the final gate passed) |
| [KK-035](KK-035.toml) | The Ledger form is prefilled from a probe row left inconclusive or manual | M6.G6 | merged `bfddeb9` |
| [KK-036](KK-036.toml) | The Cell View window over `cell` | M7.W2 (next-sprint item 5) | merged `1f0eee0` |
| [KK-037](KK-037.toml) | Cell View and Render Window in step: open a cell in the viewport, one selection | M7.R2 selection sync | merged `4275765` |
| [KK-038](KK-038.toml) | Table columns resize by dragging the header's edge | M6.V5 rest | merged `a4aaf48` |
| [KK-039](KK-039.toml) | Raw JSON held in one buffer and sliced by the visible range | M6.P5a rest | merged `fe7aea5` |
| [KK-040](KK-040.toml) | The workbench loads the last profile at start, from the index cache | M6.P3a rest | merged `875c070` |
| [KK-041](KK-041.toml) | Measure whether opt-level 2 for dependencies makes debug runs usable | M6.P6a rest | merged `89b13e2` (review: the doc says one clean build each; result: do not adopt, +77% fresh build) |
| [KK-042](KK-042.toml) | `game.session` reports progress: preflight, each phase, each probe row | M6.G4 ("progress events for its own steps") | merged `13ce441` |
| [KK-043](KK-043.toml) | MCP: a game session started over MCP is followed and stopped across calls | CLI/MCP/UI parity for M4.T3's commands | merged `db3afb9` (review: em dashes out of the new docs/mcp.md lines) |
| [KK-050](KK-050.toml) | Draft the KK-002 follow-ups: QUST's condition slots, `new` for a WRLD (2 drafts) | KK-002's follow-ups to cut | merged `9fb420c`; drafts promoted as KK-220 and KK-221 (review: briefs updated for KK-002 having merged, QUST evidence check made exact) |
| [KK-051](KK-051.toml) | Draft three apparel tickets: ARMO addons in `inspect`, LMSW `.mat` paths, armor world models | "next in engine and data" (below) | merged `dd29fd3` (QA: CHANGES, fixed at promotion); armo-inspect-addons promoted as KK-222 (brief: where `lmsw` attaches, `lo.resolve`), armor-world-models as KK-223; lmsw-mat-exists rejected (vanilla materials live in the `.cdb`, so the rule would warn on every swap to a vanilla material) |
| [KK-052](KK-052.toml) | Draft two schema tickets: inventory definitions for MRPH, CNDF, VTYP, MTPT, ASPC; type AVIF | "next in engine and data" (below) | merged `368342b` (review: committed the run's uncommitted parity-row fix); promoted as KK-224 (inventory) and KK-225 (AVIF) |
| [KK-090](KK-090.toml) | Schema tickets merge cleanly: the defs test derives its record list, shared lists one entry per line | the generated-file overlap (above, "Generated tickets") | merged `66108e2` |
| [KK-100](KK-100.toml) | Inventory definitions for SFTR, TRNS, PDCL, EFSQ and STMP | generated by plan/tickets/generate.py inventory | merged `ddf4d28` (6/7: scope failed only because the run rebased `kk/KK-100-wip` onto main, so the grader diffed against the old base; gate green; no QA ran; squashed with the 2026-10-27 run, whose harness WIP commit does not compile alone; SFTR, TRNS, PDCL, EFSQ, STMP defined, `inventory-check` clean) |
| [KK-101](KK-101.toml) | Inventory definitions for ATMO, KSSM, BMOD, FFKW and LTEX | generated by plan/tickets/generate.py inventory | merged `60fd7f0` (review `d39e576`: the run left the golden seed stale, 3/6; regenerated, STATUS line added; the gap-matrix headline outside `owns` is `cargo xtask parity` output, kept) |
| [KK-102](KK-102.toml) | Inventory definitions for LVLB, AMBS, IPDS, IMGS and MUST | generated by plan/tickets/generate.py inventory | merged `326cd04` (review `80f721e`: its five types added to `BX2_INVENTORY`; scope failed only on the gap-matrix headline `cargo xtask parity` rewrites, kept; generated files rebuilt in the merge) |
| [KK-103](KK-103.toml) | Inventory definitions for CAMS, MATT, IMAD, AACT and SPCH | generated by plan/tickets/generate.py inventory | merged `243836f` (QA: CHANGES, then ACCEPT; generated files rebuilt in the merge) |
| [KK-104](KK-104.toml) | Inventory definitions for GRAS, WTHS, STND and CSTY | generated by plan/tickets/generate.py inventory | merged `c13ed20` (QA: CHANGES; review `2838ed1`: CSTY's CSSM member named `Cssm`, was `Casm`; generated files rebuilt) |
| [KK-105](KK-105.toml) | Inventory definitions for LGDI, DLBR, FSTP, MUSC and STBH | generated by plan/tickets/generate.py inventory | merged `57b9c5e` (QA: CHANGES, then ACCEPT; generated files rebuilt in the merge). LGDI and STBH are `unordered = true` on corpus order alone, not xEdit's marking as defs/README.md asks: revisit when a family lane types them |
| [KK-106](KK-106.toml) | Inventory definitions for GCVR, FOGV, ADDN, EFSH and BMMO | generated by plan/tickets/generate.py inventory | merged `ae860fb` (QA: ACCEPT; generated files rebuilt in the merge) |
| [KK-107](KK-107.toml) | Inventory definitions for COLL, SFPC, CLMT, PTST, VOLI, MOVT, REVB and CUR3 | generated by plan/tickets/generate.py inventory | merged `bd58e2d` (QA: ACCEPT; generated files rebuilt in the merge) |
| [KK-108](KK-108.toml) | Inventory definitions for SECH, STAG, LENS, EQUP, AAPD, AOPS, CLDF and AOPF | generated by plan/tickets/generate.py inventory | merged `5c2cd7c` (QA: ACCEPT; generated files rebuilt with `add-def`; review `97309e2`: SECH's group renamed `Echoes`, its source cites the ECHO lead's counts so `inventory-check` passes) |
| [KK-109](KK-109.toml) | Inventory definitions for LVSC, FSTS, IDLM, SCOL, BNDS, TODD, CLFM and AORU | generated by plan/tickets/generate.py inventory | merged `9d906da` (6/7, no QA: scope failed only because the grader diffed against `kk/KK-109-wip`'s old base and counted main's changes merged into the branch; the net diff against main is inside `owns`; the 2026-10-28 WIP commit squashed with the run into one commit) |
| [KK-110](KK-110.toml) | Inventory definitions for PSDC, GBFT, NAVI, AAMD, LGTM, WBAR, FORC and GPOG | generated by plan/tickets/generate.py inventory | merged `6d05c51` (gate failed only on NAVI missing from bridge.rs's EDID-less list, outside the run's owns; the merge's `add-def` adds it to `WITHOUT_EDID`; `inventory-check` passes) |
| [KK-111](KK-111.toml) | Inventory definitions for DEBR, OSWP, WTHR, PERS, SDLT, DOBJ, GWED and MAAM | generated by plan/tickets/generate.py inventory | merged `c85134b` (scope failed on the run's own `xtask inventory-check`, dropped for main's, which passes; PERS added to `WITHOUT_EDID` by `add-def`) |
| [KK-112](KK-112.toml) | Inventory definitions for NOCM, OVIS, SCCO and SPGD | generated by plan/tickets/generate.py inventory | merged `5eacf2d` (the run stopped at minute 25 on a base without the tools, 5/7; review `cd8dbcc`: SPGD drops a BfcbBlock no official record has, NOCM's groups cite their lead's counts, STATUS line; parity regenerated in the merge) |
| [KK-200](KK-200.toml) | Type ALCH (Ingestible) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `86c5883` (scope failed only on a scratch `.update-golden.sh` in the harness WIP commit, left out; gap-matrix headline is parity output, kept; ALCH 31 typed, 1 opaque, 0 unknown) |
| [KK-201](KK-201.toml) | Type BOOK (Book) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `b1edf9c` (5/6: scope failed only on the gap-matrix headline, parity output outside the owns, replaced by the merge's regeneration; BOOK 25 typed, 1 opaque, 0 unknown; `book_types_full` 1368/1368 on main) |
| [KK-202](KK-202.toml) | Type DOOR (Door) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `74a5bbe` (5/6: scope failed only on a scratch `.tmp/update_golden.sh` in the harness WIP commit; review `1a9ce9a` drops it and adds the STATUS line the run left out; DOOR 26 typed, 4 opaque, 2 unknown (FLTR, INTV inside component blocks); `door_types_full` 376/376 on main) |
| [KK-203](KK-203.toml) | Type WATR (Water) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `9d0fdf5` (6/6, QA ACCEPT; WATR 17 typed, 2 opaque, 0 unknown; `watr_types_full` 19/19 on main) |
| [KK-204](KK-204.toml) | Type EXPL (Explosion) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `22ef15b` (5/6: scope failed only on `docs/foothold/gap-matrix.md`, owned since `c1f9b6e`; no QA ran; WIP commit (rustfmt) folded into a review commit; EXPL 17 typed, 2 opaque, 0 unknown; `expl_types_full` 359/359) |
| [KK-205](KK-205.toml) | Type HAZD (Hazard) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `d33801d` (5/6: scope failed only on `docs/foothold/gap-matrix.md`, owned since `c1f9b6e`; no QA ran; HAZD 14 typed, 2 opaque, 0 unknown; `hazd_types_full` 87/87) |
| [KK-206](KK-206.toml) | Type LIGH (Light) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `13b1a2e` (5/6: the gate failed on xtask's `families_name_defined_types_once`, which named LIGH as its sample inventory while the brief kept `xtask/src/parity.rs` out of scope; the review commit points it at LAYR, folds the run's uncommitted work and renames FLBD `HasFallout` to `HasFalloff`; no QA ran; LIGH 21 typed, 1 opaque, 2 unknown (FLTR, INTV inside component blocks); `ligh_types_full` 1,723/1,723) |
| [KK-207](KK-207.toml) | Type KEYM (Key) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `1ca1507` (3/6, no QA: codegen-fresh, schema-all and the gate failed in the grader only, which copies the agent's working tree; on the committed tree codegen is fresh, kitkraft-schema passes, keym_types_full prints 343 identical and clippy is clean; regenerated with the night's other schema merges) |
| [KK-208](KK-208.toml) | Type HDPT (HeadPart) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `2064794` (6/6, QA ACCEPT) |
| [KK-209](KK-209.toml) | Type ENCH (Enchantment) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `f693f1e` (6/6, QA ACCEPT) |
| [KK-210](KK-210.toml) | Type ARTO (ArtObject) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `7d46b49` (6/6, QA ACCEPT) |
| [KK-211](KK-211.toml) | Type TXST (TextureSet) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `2c8ddf8` (6/6, QA ACCEPT) |
| [KK-212](KK-212.toml) | Type BPTD (BodyPartData) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `aecc9b7` (6/6, QA ACCEPT) |
| [KK-213](KK-213.toml) | Type PGRE (PlacedProjectile) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `dd74547` (6/6, QA CHANGES: only that the second round was staged, not committed; the harness fixup carried it, autosquashed) |
| [KK-214](KK-214.toml) | Type PHZD (PlacedHazard) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `b269ee2` (6/6, QA ACCEPT; review commit `35e7c0c`: STATUS wording of the member order) |
| [KK-215](KK-215.toml) | Type IRES (Resource) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `6f95b2f` (5/6, gate failed on rustfmt only, no QA; review commit `cb0d9d6`: rustfmt, the IRES gap-matrix row, STATUS line) |
| [KK-216](KK-216.toml) | Type MESG (Message) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-217](KK-217.toml) | Type GPOF (GameplayOption) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-218](KK-218.toml) | Type SOUN and WKMF from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-219](KK-219.toml) | Type RFGP (ReferenceGroup) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-220](KK-220.toml) | `new` writes every condition group of a QUST: align CTDA occurrence counts with the slots | drafted by the local agent (KK-050) | merged `73c2c22` (6/6, QA ACCEPT after one CHANGES; the plan walks the chosen layout's nested groups and `new` holds only the selected layout's block slots; QUST typed-writable pass, plus a ContactShadow read-back test) |
| [KK-221](KK-221.toml) | `new` writes a WRLD: a lone worldspace in its top-level group, no cells yet | drafted by the local agent (KK-050) | queued (KK-220 merged 2026-10-29) |
| [KK-222](KK-222.toml) | `inspect` shows an ARMO's addons and each addon's model paths | drafted by the local agent (KK-051) | merged `388ffa5` (4/4, QA ACCEPT) |
| [KK-223](KK-223.toml) | validate checks the ARMO and ARMA world and biped model files | drafted by the local agent (KK-051) | merged `3045ddc` (overtime run, 6/6, QA ACCEPT; official `asset.missing-model` 628: Starfield.esm 603, ShatteredSpace.esm 16; also pins the reserved DOOR ActorValue KK-202 exposed) |
| [KK-224](KK-224.toml) | Inventory definitions for MRPH, CNDF, VTYP, MTPT and ASPC | drafted by the local agent (KK-052) | queued (KK-023 merged 2026-10-29) |
| [KK-225](KK-225.toml) | Type AVIF (actor value information) from the pinned xEdit file, corpus-checked | drafted by the local agent (KK-052) | after KK-224 (owns: gap matrix added 2026-10-29, as KK-023's scope miss showed) |
| [KK-226](KK-226.toml) | Type WTHR (Weather) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-227](KK-227.toml) | Type LGDI (LegendaryItem) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-228](KK-228.toml) | Type CSTY (CombatStyleForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-229](KK-229.toml) | Type SECH (SoundEchoMarker) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-230](KK-230.toml) | Type IPCT (Impact) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-231](KK-231.toml) | Type LVLB (LeveledBaseForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-232](KK-232.toml) | Type MUST (MusicTrack) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-233](KK-233.toml) | Type BMMO (BiomeMarker) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-234](KK-234.toml) | Type MATT (MaterialType) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-235](KK-235.toml) | Type STND (SnapTemplateNode) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-236](KK-236.toml) | Type ADDN (AddonNode) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-237](KK-237.toml) | Type LENS (LensFlare) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-238](KK-238.toml) | Type AVMD (AvmsData) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-239](KK-239.toml) | Type SPCH (SpeechChallengeForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-240](KK-240.toml) | Type CAMS (CameraShot) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-241](KK-241.toml) | Type MUSC (MusicTypeForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-242](KK-242.toml) | Type COLL (CollisionLayerForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-243](KK-243.toml) | Type MOVT (MovementTypeForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-244](KK-244.toml) | Type AAPD (AimAssistPoseDataForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-245](KK-245.toml) | Type REGN and GRAS from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |

On 2026-10-25 KK-002 and KK-006 failed to start: the harness did not resolve their WIP base
branch (fixed in the harness). Both are requeued unchanged, still continuing from their WIP
branches; KK-008 still waits for KK-006 to merge. The same night KK-005 (run 2) and KK-007
landed.

On the 2026-10-25 13:30 night KK-006 and KK-010 merged; KK-002 is requeued from its WIP
branch (QUST still fails; ignore it and finish the evidence). KK-008 is runnable now.

KK-002 and KK-004 both add a section to `docs/schema/support-matrix.md`, and KK-004 and
KK-006 both may add a row to `docs/gaps.md`. Merge one, then resolve the other by hand.
Schema tickets read the pinned xEdit file at `/opt/xedit-ref` (`kk-setup` binds
`<checkout>\..\tools\xedit-ref` when it exists).

KK-007 and KK-008 are the read half of apparel request 7 and do not depend on each other
(KK-007 reads `REFL` bytes whether or not `LMSW` has a definition). KK-008 shares schema
and generated-code paths with KK-006 (ARMA), so queue it after KK-006 has merged. Writing an `LMSW` (`set`
on its swap paths, or `new` from entries) is cut after KK-007 lands, on its decoder.

On the 2026-10-26 06:10 night KK-011, KK-012, KK-013, KK-014, KK-015 and KK-018 merged
(KK-011, KK-013 and KK-018 with review commits). KK-019 still waits on KK-008; the ARMO
addons and world-model tickets in "next in engine and data" now have KK-018's rule.

On the 2026-10-27 02:38 night KK-008, KK-016, KK-017, KK-031, KK-037 and KK-039 merged
(KK-017 with a review commit on the gap matrix). KK-022 is requeued from its WIP branch;
KK-023 still waits on it. KK-019, KK-021 and KK-032 are runnable now.

KK-010 to KK-023 (engine and data, cut 2026-10-25). Independent, any night: KK-010 to KK-015;
their `owns` do not overlap each other or the queued tickets, except that KK-014 shares
`docs/schema/support-matrix.md` with KK-002 and `docs/gaps.md` with KK-006 (sections and
rows only; merge one, resolve the other by hand). Chains, by `after`:

- typed-write: KK-002 -> KK-017 -> KK-021 (KK-021 also waits on KK-006 for `ARMA` and the
  `BO64` flags). All three own `authoring_tests.rs` and `docs/schema/evidence.toml`, hence the
  chain.
- gap matrix: KK-002 -> KK-016 (it fixes the hand table where KK-002's evidence changed MX).
- validate: KK-005 -> KK-018 (both own `crates/engine/src/validate/` and the official-files
  test).
- material swaps: KK-007 + KK-008 -> KK-019 -> KK-020 (KK-019 owns `set.rs`, `newform.rs` and
  `crates/engine/src/lib.rs`; KK-020 owns `build/spec.rs` and `handlers/build.rs`).
- schema: until KK-090 merges, schema tickets run one per night (they all regenerate
  `crates/esm/src/generated/` and the golden seed, and rustfmt reflows the shared test lists).
  Once KK-090 has made those lists one entry per line, schema tickets on different record types
  may run the same night: the pick ignores their overlap on the regenerated and sorted-list
  files, and the merge rebuilds them ("Generated tickets" above). KK-022 and KK-023 no longer
  chain on that account.

`docs/record-parity.md` is generated: KK-016, KK-022 and KK-023 all own it and may run the
same night. Merge in any order and rerun `cargo xtask parity` on the merged result; never
merge it by hand. KK-016 changes `xtask/src/parity.rs`, so merge it before regenerating for
the others.

Decisions taken in the tickets (overturn before queueing if wrong): KK-019's argument name
`swap_overrides` and its rule that an override must end in `.mat`; KK-018's severity
(warning) and its scope (`Model.FileName` only); KK-010's probe `player.getequipped`
(answer format grade D, like the other console probes).

Still to cut, from `docs/foothold/next-sprint.md`: the Cell View's engine-facing tests (item 5,
the other lane). Next in engine and data once their inputs land: `inspect` of an ARMO's
addons and their model paths (after KK-006, KK-018); `validate` of `LMSW` override paths
against shipped `.mat` files, existence only (after KK-018, KK-019); the world-model members
of ARMO and ARMA in `asset.missing-model`; more inventory definitions (MRPH, CNDF, VTYP,
MTPT, ASPC) and lstring-only types (AVIF). The hand-written half of the gap matrix (its
evidence prose) and the `.mat` content rules stay with Claude or the owner. KK-051 and
KK-052 have the local agent draft these five, and KK-050 KK-002's two follow-ups ("Drafts"
above); none of the three waits on anything, and the drafts carry the `after` lines.

KK-030 to KK-043 are the workbench and surface lane (M6 lanes V, P and G, M7 lanes W and R,
`crates/mcp`, `crates/game` through its fakes). Every UI ticket is tested headless: view-model
and shell-model tests, and egui frames run on a bare `egui::Context`. The game-loop chain is
KK-030 (the engine's `testing::GameFakes`, so `ui-egui` and `crates/mcp` can drive
`game.*` against the fake console and host without depending on `kitkraft-game`), then
KK-031 (gate), KK-032 (launch and follow), KK-033 (rows and screenshots), KK-034
(`game.probe`, and the Ledger button that uses KK-035's prefill). KK-042 and KK-043 also
wait on KK-030 only; KK-037 waits on KK-036. Independent on a first night: KK-030, KK-035,
KK-036, KK-038, KK-039, KK-040, KK-041. The Cell View (next-sprint item 5) is KK-036 and
KK-037.

Overlaps. Shared docs, merged by hand: `docs/workbench.md` (KK-031 to KK-038, KK-040),
`docs/design/workbench-performance.md` (KK-039, KK-040, KK-041), `docs/ck-parity.md`
(KK-036, KK-037). Code paths two tickets own are kept on different nights by `after`, except
where a requeue could bring them together: `menu.rs` (KK-036, then KK-031 and KK-034),
`panels/common.rs` (KK-038, then KK-032 and KK-034), `app.rs` (KK-040, then KK-032 and
KK-034), `windows/shell.rs` and `windows/cell_view.rs` (KK-036, then KK-037). If KK-036,
KK-038 or KK-040 is requeued, do not queue it with the later ticket that shares its file;
the hunks differ, so a hand merge is short. KK-030 and KK-042 both own
`crates/engine/src/handlers/game/mod.rs` (KK-042 runs after KK-030). KK-030 adds one line
to `crates/engine/src/lib.rs` and moves `GameHost` out of `handlers/game/mod.rs` so that
`crates/engine/src/handlers.rs` (lane D's) stays untouched.

Not ticketable in this lane, for the local agent: anything that needs the window driven
(M7.R1's V1, M6.P1's window rows L5, I1, I3, I4, T4 and U1, the screenshot tour), the GPU,
a CK sitting (M7.K3, M7.K4, then M7.W1, M7.M2, M7.M3, M7.M5, M7.S1, M7.S2, M7.W4), the
owner's rulings (M7.E2 to M7.E6 on M7.O4; M7.R3's NIF variant, M7.R4, M7.R5, M7.S4, M7.S5 on
M7.O5), or a live game session (M6.G4's V1 run from the window, M6.O4). Lane X is DEFER.
