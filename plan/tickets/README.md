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
  "crates/engine/src/handlers/",  # never plan/STATUS.md: tickets don't write it
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
- **No "Landed" line** (retired 2026-11-15). A ticket does not write `plan/STATUS.md`: its
  merge commit and its row here are the record, and the agent puts what it found (counts its
  test printed, what stays open) in its commit message. A fact a later reader needs goes in
  the doc it belongs to (`docs/schema/support-matrix.md`, `docs/gaps.md`, a design note),
  owned by the ticket. `lint.py` warns on a brief that asks for a Landed line. The grader
  still allows a change to `plan/STATUS.md` (a run from an older prompt may add a line);
  the merge deletes such a line.
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
hand. `overnight kitkraft KK-002+KK-003 8` runs a list instead.

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
| [KK-216](KK-216.toml) | Type MESG (Message) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `f454e76` (4/6: schema-all and gate failed on a stale golden seed, work uncommitted, no QA; review commit `7cad8ba`: squashed the harness wip commit, golden seed, gaps counts, STATUS line) |
| [KK-217](KK-217.toml) | Type GPOF (GameplayOption) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `902f9c6` (6/6, QA CHANGES: its points were the staged docs, now in the autosquashed fixup, the scratch probe, left out, and the record flags and VNAM/WNAM names; the flags are xEdit's L12369-L12384 and the names follow xEdit's two identical 'Default Value' labels, both kept) |
| [KK-218](KK-218.toml) | Type SOUN and WKMF from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `549f433` (4/6: reconcile failed on the gap-matrix audio row, gate on rustfmt, work uncommitted, no QA; review commit `51f6bf0`: squashed the harness wip commit, rustfmt, audio row split into SOUN/WKMF yes and WWED no with the tally, gaps rows, STATUS line) |
| [KK-219](KK-219.toml) | Type RFGP (ReferenceGroup) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `8888cdf` (5/6: gate failed on rustfmt only, no QA; review commit `1c78076`: rustfmt, the corpus evidence line from rfgp_types_full, gaps rows, STATUS line) |
| [KK-220](KK-220.toml) | `new` writes every condition group of a QUST: align CTDA occurrence counts with the slots | drafted by the local agent (KK-050) | merged `73c2c22` (6/6, QA ACCEPT after one CHANGES; the plan walks the chosen layout's nested groups and `new` holds only the selected layout's block slots; QUST typed-writable pass, plus a ContactShadow read-back test) |
| [KK-221](KK-221.toml) | `new` writes a WRLD: a lone worldspace in its top-level group, no cells yet | drafted by the local agent (KK-050) | merged `e8858e3` (5/6: gate red only on `new_and_clone_refusals_are_typed`'s WRLD case, which the ticket's 2026-11-01 `owns` fix allows; changed to INFO in the review commit; both WRLD typed-write tests un-ignored) |
| [KK-222](KK-222.toml) | `inspect` shows an ARMO's addons and each addon's model paths | drafted by the local agent (KK-051) | merged `388ffa5` (4/4, QA ACCEPT) |
| [KK-223](KK-223.toml) | validate checks the ARMO and ARMA world and biped model files | drafted by the local agent (KK-051) | merged `3045ddc` (overtime run, 6/6, QA ACCEPT; official `asset.missing-model` 628: Starfield.esm 603, ShatteredSpace.esm 16; also pins the reserved DOOR ActorValue KK-202 exposed) |
| [KK-224](KK-224.toml) | Inventory definitions for MRPH, CNDF, VTYP, MTPT and ASPC | drafted by the local agent (KK-052) | merged `4cb09c7` (6/7: gate red only on the ui-egui Windows-path harness tests, fixed on main by `21f874d`; inventory-check passes for all five) |
| [KK-225](KK-225.toml) | Type AVIF (actor value information) from the pinned xEdit file, corpus-checked | drafted by the local agent (KK-052) | merged `feed136` (5/6: run gate red only on main's Linux-only `mesh_tests::a_loose_model_wins_over_the_archives`; no QA ran; review: AVIF leaves out xEdit's base form component block, absent from the corpus) |
| [KK-226](KK-226.toml) | Type WTHR (Weather) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `81f4ef4` (6/6, QA ACCEPT; DATA and the rest checked against the pinned xEdit lines; the Sounds group is in no official record, reference only) |
| [KK-227](KK-227.toml) | Type LGDI (LegendaryItem) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `f63fc0e` (6/6, QA CHANGES: parity and rustfmt left uncommitted, folded from the harness fixup by autosquash; the Landed line added in the review commit; typed members stay at `reference` confidence) |
| [KK-228](KK-228.toml) | Type CSTY (CombatStyleForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `ca743fb` (5/6: run gate red only on main's Linux-only mesh test; no QA ran; review: the Landed line's run-gate note dropped at merge) |
| [KK-229](KK-229.toml) | Type SECH (SoundEchoMarker) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `1296274` (5/6: run gate red only on main's Linux-only mesh test; no QA ran; review: the Landed line's run-gate note dropped at merge) |
| [KK-230](KK-230.toml) | Type IPCT (Impact) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `53cca8f` (5/6: run gate red only on main's Linux-only mesh test; no QA ran) |
| [KK-231](KK-231.toml) | Type LVLB (LeveledBaseForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `5152b31` (4/6: harness fixup folded by autosquash; review commit `3df6575` reverts the out-of-scope `crates/engine/src/handlers/mesh.rs` change; run gate red on the Linux-only ui-egui `preferences_change_the_zoom_theme_and_checkout`, which the ticket does not touch) |
| [KK-232](KK-232.toml) | Type MUST (MusicTrack) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `3d91192` (3/6 at the deadline: must-full and parity-rows passed, the corpus evidence edits left records.rs, seed.rs and the parity docs stale; harness fixup folded by autosquash; review commit `509140f` regenerates them and restores three per-member xEdit citations) |
| [KK-233](KK-233.toml) | Type BMMO (BiomeMarker) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `5c8f20b` (6/6; both later QA runs died on an API error, so no final verdict: the merge reviewer checked it against the xEdit lines; review commit `2d19f5f` makes the test fail on zero records) |
| [KK-234](KK-234.toml) | Type MATT (MaterialType) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `baabe7f` (6/6, QA ACCEPT) |
| [KK-235](KK-235.toml) | Type STND (SnapTemplateNode) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `23db411` (6/6, QA ACCEPT; review commit `e11a78a` makes the test fail on zero records) |
| [KK-236](KK-236.toml) | Type ADDN (AddonNode) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `b77bbfc` (6/6, QA ACCEPT; review commit `c92f993` drops an uncited claim from the ADDN header) |
| [KK-237](KK-237.toml) | Type LENS (LensFlare) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `a8b7826` (6/6, QA ACCEPT; facts checked against the pinned file at the merge) |
| [KK-238](KK-238.toml) | Type AVMD (AvmsData) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `f4421d0` (3/6: the run hit its deadline with the corpus test passing but the parity docs stale and no Landed line; review commit `ae45660` adds the corpus evidence and the Landed line, the merge regenerates the docs: `avmd_types_full` 6530 of 6530, parity 12 typed, 3 unknown; no QA ran) |
| [KK-239](KK-239.toml) | Type SPCH (SpeechChallengeForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `fac185d` (6/6, QA ACCEPT; facts checked against the pinned file at the merge) |
| [KK-240](KK-240.toml) | Type CAMS (CameraShot) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `15c9f6b` (6/6, QA CHANGES for the missing Landed line, the uncommitted work having reached the branch in the harness's fixup; review commit `a584e16` adds the line and restores corpus confidence on Keywords) |
| [KK-241](KK-241.toml) | Type MUSC (MusicTypeForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `81644dc` (6/6, QA CHANGES for the header comment calling the base form components raw and TNAM's corpus count living only in its note; review commit `5aa289d` fixes both; the Landed line reached the branch in the harness's fixup, autosquashed) |
| [KK-242](KK-242.toml) | Type COLL (CollisionLayerForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `f35e005` (6/6, QA CHANGES for the missing Landed line; review commit `6091ee6` adds it and points CNAM's corpus line at the count coll_types_full printed; merge conflicts in generated files regenerated) |
| [KK-243](KK-243.toml) | Type MOVT (MovementTypeForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `7cc55b3` (6/6, QA ACCEPT; review commit `6c0e4c0` rewrites the BOLV note, which read the marker backwards against xEdit's CK comment: present means Display Character Only is off) |
| [KK-244](KK-244.toml) | Type AAPD (AimAssistPoseDataForm) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `0cd2d73` (6/6, QA ACCEPT; no review commit; merge conflicts in generated files regenerated) |
| [KK-245](KK-245.toml) | Type REGN and GRAS from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | merged `bad9353` (4/6 from `kk/KK-245-wip`: schema-all and the gate failed only on a stale golden seed after the run's last commit made RCLR required; review commit `8d56c0a` regenerates it and adds the missing Landed line; the harness's wip commit folded into the run's first by hand; merge conflicts in generated files regenerated) |
| [KK-246](KK-246.toml) | Workbench: show a form's refusal next to the button that caused it | workbench UX audit, finding A1 (`docs/design/workbench-ux.md`) | merged `3463582` (4/4, QA CHANGES: refusals were keyed by egui's automatic button ids, which move when a line appears above the button; review commit `4af0123` keys them by a stable name per button and adds `audit_a1_a_refusal_stays_by_its_button_when_a_line_appears_above`) |
| [KK-247](KK-247.toml) | Workbench: give the two copy buttons of a result block distinct labels | workbench UX audit, finding A5 | merged `a1ea069` (4/4, QA ACCEPT; the Warnings window keeps its own "copy" button, outside A5) |
| [KK-248](KK-248.toml) | Workbench: open the CK-style windows where they cover neither the tab nor each other | workbench UX audit, finding A6 | merged `9a1c752` (4/4; QA did not run: OAuth session expired; reviewed at the merge in its place, no review commit) |
| [KK-249](KK-249.toml) | Workbench: make a long path in a single-line field readable in full | workbench UX audit, finding A7 | merged `53146ff` (4/4; QA did not run: OAuth session expired; reviewed at the merge in its place, no review commit) |
| [KK-250](KK-250.toml) | testplan: a reference placed from a pack-in no longer expects the PKIN as its base | owner decision 2026-11-05, item 9 (T-20 of `game-20261103-03` and `-08`) | merged `7a637b5` (4/4, QA ACCEPT; released by the owner 2026-11-07, ruling 4). The sprint 2026-11-09 did it too (`0d768c3d`, fix `e709267a`); the sprint merge kept this one and dropped the sprint's |
| [KK-251](KK-251.toml) | testplan: check a placed actor (ACHR) by its cell and a distance, not its exact position | owner decision 2026-11-05, item 9 (T-04 and T-06 of `game-20261103-06`) | merged `2e1f8665` (sprint 2026-11-09, Opus writer; review `reviews/r-R1fix-R2-R3.md`), on main through the sprint merge (2026-11-15); the ticket has `hold` so the picker does not run it again |
| [KK-252](KK-252.toml) | Type ASPC (AcousticSpace) from the pinned xEdit file, corpus-checked | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `487e23c3` (Opus lane `opus/schema-depth-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `2060e072`); the ticket has `hold` so the picker does not run it again |
| [KK-253](KK-253.toml) | Type PDCL (ProjectedDecal) from the pinned xEdit file, corpus-checked | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `c094c5a1` (Opus lane `opus/schema-depth-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `2060e072`); the ticket has `hold` so the picker does not run it again |
| [KK-254](KK-254.toml) | Type STMP (SnapTemplateForm) from the pinned xEdit file, corpus-checked | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `d7781e3b` (Opus lane `opus/schema-depth-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `2060e072`); the ticket has `hold` so the picker does not run it again |
| [KK-255](KK-255.toml) | Type IDLM (IdleMarker) from the pinned xEdit file, corpus-checked | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `60338c02` (Opus lane `opus/schema-depth-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `2060e072`); the ticket has `hold` so the picker does not run it again |
| [KK-256](KK-256.toml) | Type SCOL (StaticCollection) from the pinned xEdit file, corpus-checked | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `33ec225a` (Opus lane `opus/schema-depth-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `2060e072`); the ticket has `hold` so the picker does not run it again |
| [KK-257](KK-257.toml) | Type CNDF and LVSC from the pinned xEdit file, corpus-checked | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `18f5c07e` (Opus lane `opus/schema-depth-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `2060e072`); the ticket has `hold` so the picker does not run it again |
| [KK-258](KK-258.toml) | Typed-write tests for ALCH, BOOK and KEYM | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `b5dd24a5` (Opus lane `opus/typed-write-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `720c8d76`); the ticket has `hold` so the picker does not run it again |
| [KK-259](KK-259.toml) | Typed-write tests for FLOR, DOOR and LIGH | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `92c307cc` (Opus lane `opus/typed-write-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `720c8d76`); the ticket has `hold` so the picker does not run it again |
| [KK-260](KK-260.toml) | Typed-write tests for FURN, MSTT and MESG | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `ca73941e` (Opus lane `opus/typed-write-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `720c8d76`); the ticket has `hold` so the picker does not run it again |
| [KK-261](KK-261.toml) | Clone-sample corpus tests for ALCH and BOOK | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `d98b6665` (Opus lane `opus/validate-corpus-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `53531a13`); the ticket has `hold` so the picker does not run it again |
| [KK-262](KK-262.toml) | validate warns when a master-flagged plugin lists a master without the master flag | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `ce5bc088` (Opus lane `opus/validate-corpus-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `53531a13`); the same rule came with sprint/2026-11-09 (`23d62954`); the ticket has `hold` so the picker does not run it again |
| [KK-263](KK-263.toml) | Workbench: a harness test that no tab has two buttons with one label (UX principle 3) | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | queued (hold removed 2026-11-18 on `plan/holds-20261118`: brief checked against main `f275b5c9`; `click` is in `tests.rs`, the Landed line dropped; `audit_every_tab` there found no duplicate label on the 15 tabs, so the new test can pass) |
| [KK-264](KK-264.toml) | docs/gaps.md: fix the four rows that main already contradicts | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `3dbeb92a` (KK-321, Opus lane `opus/house-20261115`, night of 2026-11-15, Opus writer, not the local agent: the four `docs/gaps.md` rows; on main through its merge `91a694ee`; rechecked 2026-11-18 at `f275b5c9`: no `PLANNED_IN_M2` row, one "Group type 10" row, no "queued as KK-224", no stale lstring-IDs row); the ticket has `hold` so the picker does not run it again |
| [KK-265](KK-265.toml) | Typed-write tests for ENCH, IRES and HAZD | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `ddc7bebf` (Opus lane `opus/typed-write-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `720c8d76`); the ticket has `hold` so the picker does not run it again |
| [KK-266](KK-266.toml) | Clone-sample corpus tests for MISC and DOOR | drafted 2026-11-13 (`plan/ticket-drafts-20261113`, never committed until KK-323) | merged `abeb55e3` (Opus lane `opus/validate-corpus-20261114`, night of 2026-11-14, Opus writer, not the local agent; on main through its merge `53531a13`); the ticket has `hold` so the picker does not run it again |
| [KK-290](KK-290.toml) | Type WWED (WwiseEventData) from the pinned xEdit file, corpus-checked | written 2026-11-15 in the shape of KK-244 and KK-245 | merged `8b1a0a6` (7/7; QA did not run: OAuth session expired; reviewed at the merge in its place, facts checked against the pinned file; the run committed nothing, so the harness's wip commit was squashed by hand into `0d9607d`) |
| [KK-291](KK-291.toml) | Typed-write tests for WWED; the gap matrix marks it typed-writable | written 2026-11-15 with KK-290 | merged `db37516` (6/6; QA did not run: OAuth session expired; reviewed at the merge in its place, no review commit; the merge kept main's DIAL and INFO entries beside WWED and reran `cargo xtask parity`) |
| [KK-330](KK-330.toml) | Type EQUP (EquipType) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-290's shape (inventory to typed) | merged `93a8481a` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-331](KK-331.toml) | Type FSTP (Footstep) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-290's shape | merged `b0394497` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-332](KK-332.toml) | Type VTYP (VoiceType) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-290's shape | merged `b5bf2b24` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-333](KK-333.toml) | Type LTEX (LandscapeTexture) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-290's shape | merged `2ef24b2d` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-334](KK-334.toml) | Type LAYR (Layer) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-290's shape; `docs/gaps.md` row "M4.BX2 inventories (LAYR LSCR)" | merged `d46a219c` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-335](KK-335.toml) | Type TRNS (Transform) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-290's shape | merged `1d506f4e` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-336](KK-336.toml) | Type MRPH (MorphableObject) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-290's shape | merged `8f44c057` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-337](KK-337.toml) | Type DMGT (DamageType) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-225's shape (lstring-only to typed) | merged `5fb021b0` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-338](KK-338.toml) | Type FXPD (FacialExpression) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-225's shape | merged `b67a7375` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-339](KK-339.toml) | Type PMFT (PhotoModeFeature) from the pinned xEdit file, corpus-checked | written 2026-11-15 in KK-225's shape | merged `c4ecbec2` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-340](KK-340.toml) | Typed-write tests for COLL, MATT and ADDN | written 2026-11-15 in KK-291's shape; types outside the gap matrix's family table | merged `65665153` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-341](KK-341.toml) | Typed-write tests for SPCH, STND and MOVT | written 2026-11-15 in KK-291's shape; types outside the family table | merged `29cd710a` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-342](KK-342.toml) | Clone-sample corpus test for CONT | written 2026-11-15 in KK-014's shape, one type per ticket | merged `9a8136ef` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-343](KK-343.toml) | Clone-sample corpus test for KEYM | written 2026-11-15 in KK-014's shape, one type per ticket | merged `8d7d46db` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-360](KK-360.toml) | testplan: an overridden inventory item keeps its help row and gets an additem row (R22) | written 2026-11-15 for the 2026-11-16 bench session (probe emitters; `priority = 1`) | merged `ce631828` (4/5: acceptance and scope green, gate red on one test outside `owns`, `modrun_tests`' Rename session row list, which the run named in its commit as the brief asked; QA did not run: OAuth session expired; reviewed at the merge in its place; review commit `68077628` makes that test expect the additem row; no fixup commit to fold) |
| [KK-361](KK-361.toml) | cargo xtask session-evidence: the evidence.toml entries a session report's passing rows justify | written 2026-11-15 for the 2026-11-16 bench session (`priority = 1`) | merged `5e1ee8b2` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-362](KK-362.toml) | game session: agent-sent input is recorded in report.json (ADR 0010 Amendment 2) | written 2026-11-15 for the 2026-11-16 bench session (`priority = 1`) | merged `dfd2ada4` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-363](KK-363.toml) | testplan: a new interior cell is entered by coc and checked by its runtime ID | written 2026-11-15 for the 2026-11-16 bench session (`priority = 1`) | merged `b037c057` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-364](KK-364.toml) | session-evidence: mark proposed entries that docs/schema/evidence.toml already has | written 2026-11-15 for the 2026-11-16 bench session (`priority = 1`) | merged `8420c6ce` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-365](KK-365.toml) | kit help: a list flag's synopsis says the flag repeats, as the parser takes it | written 2026-11-16 from the OW-2 rehearsal (`priority = 1`) | merged `19720c3a` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-366](KK-366.toml) | workbench: the window title no longer says read-only | written 2026-11-16 from the OW-2 rehearsal (`priority = 1`) | merged `d194356f` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-367](KK-367.toml) | deploy: the text summary names the plugin's SHA-256, as the window's preview does | written 2026-11-16 from the OW-2 rehearsal (`priority = 1`) | merged `b44b15cb` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-368](KK-368.toml) | workbench: the checkout above the working directory comes before the one above kit-ui | written 2026-11-16 from the OW-2 rehearsal (`priority = 1`) | merged `7da9bee2` (Opus lane `opus/tickets365-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-369](KK-369.toml) | game session: one echo-only launch poll no longer makes the launch wait time out | written 2026-11-16 from OW-1's runs, `game-20261116-01` (`priority = 1`) | merged `b830ae86` (Opus lane `opus/tickets361-20261117`, night of 2026-11-17, Opus writer, not the local agent; on main through the integration branch `opus/night-land-20261117`); the ticket has `hold` so the picker does not run it again |
| [KK-370](KK-370.toml) | Type DLBR (DialogBranch) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | merged `88534d09` (run `kk-KK-370-20261117-0531`, 7/7; QA did not run: OAuth expired, merge-agent review only) |
| [KK-371](KK-371.toml) | Type CLFM (ColorForm) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | merged `3aff87f8` (run `kk-KK-371-20261118-0520`, 7/7, QA ACCEPT) |
| [KK-372](KK-372.toml) | Type AORU (AttractionRuleForm) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | merged `fff1d451` (run `kk-KK-372-20261118-0520`, 7/7, QA ACCEPT; generated files regenerated after KK-371) |
| [KK-373](KK-373.toml) | Type WBAR (WeaponBarrelModel) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | merged `13b7c66b` (run `sweep-KK-373-haiku-20261117-2230`, Haiku 5.5 cloud agent, 6/7: the run's gate failed on the machine (ENOMEM); gate green on the merged tree; no QA session, checked against xEdit by hand) |
| [KK-374](KK-374.toml) | Type REVB (ReverbParameters) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | merged `8a176e26` (run `sweep-KK-374-haiku-20261117-2230`, Haiku 5.5 cloud agent, 7/7; no QA session, checked against xEdit by hand) |
| [KK-375](KK-375.toml) | Type DFOB (DefaultObject) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-376](KK-376.toml) | Type AACT (ActionForm) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-377](KK-377.toml) | Type AOPF (AudioOcclusionPrimitive) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-378](KK-378.toml) | Type BNDS (BendableSplineForm) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-379](KK-379.toml) | Type IPDS (ImpactDataSet) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-380](KK-380.toml) | Type SFPC (SurfacePatternConfig) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-381](KK-381.toml) | Type SDLT (SecondaryDamageListForm) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-382](KK-382.toml) | Type SCCO (SceneCollection) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-383](KK-383.toml) | Type CLMT (ClimateForm) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-384](KK-384.toml) | Type SPGD (ShaderParticleGeometry) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-385](KK-385.toml) | Type LGTM (LightingTemplateForm) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-386](KK-386.toml) | Type STAG (AnimationSoundTagSet) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-387](KK-387.toml) | Type AAMD (AimAssistModelData) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-388](KK-388.toml) | Type MAAM (MeleeAimAssistModel) from the pinned xEdit file, corpus-checked | written 2026-11-17 in KK-330's shape (inventory to typed); `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-389](KK-389.toml) | Typed-write tests for AMMO, PROJ and ZOOM | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-390](KK-390.toml) | Typed-write tests for ARTO, HDPT and BPTD | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-391](KK-391.toml) | Typed-write tests for EXPL, IPCT and LENS | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-392](KK-392.toml) | Typed-write tests for MUSC, MUST and SECH | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-393](KK-393.toml) | Typed-write tests for CSTY, AAPD and BMMO | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-394](KK-394.toml) | Typed-write tests for AVMD, CAMS and PDCL | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-395](KK-395.toml) | Typed-write tests for STMP, IDLM and SCOL | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-396](KK-396.toml) | Typed-write tests for CNDF, LVSC and LVLB | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-397](KK-397.toml) | Typed-write tests for WATR, GPOF and ASPC | written 2026-11-17 in KK-340's shape; MX `not-run` for typed types (calls tried on `d8c3b1c3`) | queued |
| [KK-398](KK-398.toml) | Clone-sample corpus test for ARMO | written 2026-11-17 in KK-342's shape, one type per ticket | queued |
| [KK-399](KK-399.toml) | Clone-sample corpus test for WEAP | written 2026-11-17 in KK-342's shape, one type per ticket | queued |
| [KK-400](KK-400.toml) | Clone-sample corpus test for FLOR | written 2026-11-17 in KK-342's shape, one type per ticket | queued |
| [KK-401](KK-401.toml) | Clone-sample corpus test for LIGH | written 2026-11-17 in KK-342's shape, one type per ticket | queued |
| [KK-402](KK-402.toml) | Clone-sample corpus test for ACTI | written 2026-11-17 in KK-342's shape, one type per ticket | queued |
| [KK-403](KK-403.toml) | docs/gaps.md and the defs README: the schema counts follow docs/record-parity.md | written 2026-11-17: the README's note on KK-330 to KK-339 ("Update both once after the batch") | queued |
| [KK-404](KK-404.toml) | gap matrix: the CS row names every clone-sample corpus test | written 2026-11-17: `docs/foothold/gap-matrix.md` CS row names 4 of the file's tests | queued |
| [KK-405](KK-405.toml) | Type GBFT (GenericBaseFormTemplate) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-406](KK-406.toml) | Type ATMO (Atmosphere) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-407](KK-407.toml) | Type IMGS (ImageSpaceForm) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-408](KK-408.toml) | Type WTHS (WeatherSettings) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-409](KK-409.toml) | Type OVIS (ObjectVisibilityManager) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-410](KK-410.toml) | Type DOBJ (DefaultObjectManager) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-411](KK-411.toml) | Type FSTS (FootstepSet) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-412](KK-412.toml) | Type GCVR (GroundCover) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-413](KK-413.toml) | Type GPOG (GameplayOptionsGroup) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-414](KK-414.toml) | Type PERS (UnknownPers) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-415](KK-415.toml) | Type LSCR (LoadScreen) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-416](KK-416.toml) | Type NOCM (NavmeshObstacleManager) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-417](KK-417.toml) | Type KSSM (SoundKeywordMapping) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-418](KK-418.toml) | Type AOPS (AimOpticalSightMarker) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-419](KK-419.toml) | Type EFSH (EffectShader) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-420](KK-420.toml) | Type STBH (SnapTemplateBehavior) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-421](KK-421.toml) | Type AMBS (AmbienceSetForm) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-422](KK-422.toml) | Type DEBR (Debris) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-423](KK-423.toml) | Type SFTR (SurfaceTreeForm) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-424](KK-424.toml) | Type BMOD (BoneModifier) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |
| [KK-425](KK-425.toml) | Type NAVI (NavmeshInfoMap) from the pinned xEdit file, corpus-checked, values pinned | written 2026-11-18 in KK-371's shape plus a value check; `docs/record-parity.md` "Inventory definitions" | queued |

KK-405 to KK-425 (local-model backlog, written 2026-11-18; none has `after` or `hold`):
one inventory definition each, in KK-371's shape, ordered easiest first (`priority` 2 for
KK-405 to KK-412, 1 for KK-413 to KK-420, 0 for KK-421 to KK-425, the hardest: AMBS, DEBR,
SFTR, BMOD, NAVI). Each brief says what makes its type harder than the template. New in
this batch: a value check. The corpus test also decodes one named official record and
asserts the decoded values of named fields, read from the generated types (not from raw
bytes), prints `<sig> value check: ok`, and an acceptance command greps the test file for
the expected constants, so a definition of the right width but the wrong layout fails, and
so does an edited expectation. Values were read from the official files on 2026-11-18 with a
read-only walker outside the repo. Cardinality follows the defs README rule (owner ruling
2026-11-07): required only where the pinned file requires it; the corpus decides which
members exist, their order and sizes, and overrides a requirement only where an official
record lacks the member (AMBS's sound run). Same-night and merge notes as for KK-370 to
KK-388 above: each removes its own line from `BX2_INVENTORY`; regenerated and sorted-list
files are rebuilt at the merge.

Not ticketed, from the same "Inventory definitions" list: the 14 types whose only untyped
payload is a reflection payload (`REFL`/`RDIF`: CLDF, CUR3, CURV, EFSQ, FFKW, FOGV, FORC,
GWED, IMAD, LMSW, MTPT, PSDC, TODD, VOLI), since the schema has no type for that format and
a ticket could only rename an opaque member; and OSWP and PTST, whose official records hold
nothing but EDID. ATMO, IMGS and WTHS (KK-406 to KK-408) type only their `RFDP` parent link
and keep `REFL`/`RDIF` opaque.

KK-370 to KK-404 (local-model backlog, written 2026-11-17; none has `after` or `hold`).
`priority` orders the simplest schema tickets first (KK-370 to KK-373 at 2, then the
structs and arrays at 1, the one- and two-record types at 0). Same night and merge notes:

- KK-370 to KK-388 each type one inventory definition (KK-387 and KK-388 one each of the
  aim-assist pair); like KK-330 to KK-339 they own different definitions and share only the
  regenerated and sorted-list files and Markdown, so `auto` may pick all of them together.
  Each removes its own line from `BX2_INVENTORY` (KK-375: `INVENTORY_CORPUS_ONLY`) in
  `crates/schema/tests/defs.rs`; keep the lines whose ticket did not merge, then rebuild the
  generated files ("Generated tickets" above). None owns `docs/gaps.md`: its Schema row is
  KK-403's, which counts whatever main holds when it runs, so it can run before or after.
- KK-389 to KK-397 (typed-write tests) all own `authoring_tests.rs` and
  `docs/schema/evidence.toml`, as KK-340 and KK-341 do: the picker takes one of them a
  night. Each appends at the end of the file; a hand merge keeps every ticket's tests and
  entries. The 27 `writes_and_reads_back` and 22 `dangling` calls the briefs name were run
  on `main` at `d8c3b1c3` on 2026-11-17 and passed with the plain helpers (of 56 typed
  types tried, GMST, LGDI, PGRE and PHZD failed the read-back and are left out).
- KK-398 to KK-402 (clone-sample) all own `clone_sample_corpus.rs`, as KK-342 and KK-343
  do: one a night, each run 16 to 19 minutes in release in the run and again in the grade.
- KK-404 owns the gap matrix's CS row only; the schema and typed-write tickets rewrite
  that file's generated headline. Shared Markdown, merged by hand.

KK-330 to KK-343 (local-model backlog, written 2026-11-15; none has `after` or `hold`).
Same night: the ten schema tickets KK-330 to KK-339 own different definitions and share only
the regenerated and sorted-list files and Markdown, so `auto` picks all of them together,
with one of KK-340/KK-341 and one of KK-342/KK-343. KK-340 and KK-341 both own
`authoring_tests.rs` and `docs/schema/evidence.toml`, KK-342 and KK-343 both own
`clone_sample_corpus.rs`: the second of each pair is picked the next night. Notes for the
merge:

- KK-337, KK-338 and KK-339 each remove their own name from the one-line `LSTRING_ONLY`
  list of `crates/schema/tests/defs.rs` (a three-way conflict on one line when they land
  together: keep the names whose ticket did not merge) and add to its doc comment.
- The schema tickets do not own `docs/gaps.md` (except KK-334, for the "M4.BX2 inventories
  (LAYR LSCR)" row). Its "Schema" summary row still counts 3 lstring-only definitions and
  names DMGT, FXPD, PMFT; `crates/schema/defs/README.md` "Current set" still lists LAYR as an
  inventory. Update both once after the batch.
- KK-340 and KK-341: all eleven test calls named in the briefs were tried on `main` at
  `98f94072` with the plain helpers and passed. The typed-write lane of 2026-11-15 adds
  helpers and tests to the same file; the tickets append at the end.
- KK-342 and KK-343 run their test in release (16 to 19 minutes; about 50 in debug), in the
  run and again in the grade.

KK-252 to KK-266 were drafted on 2026-11-13 and run by Opus lanes the next night instead of
the local agent; their files came to `main` later (KK-323), each with `hold`, because the picker
counts a ticket as merged only by a "Merge ticket/<ID>:" commit. KK-267 to KK-289 (the
conditions, dialogue and component-block lanes of 2026-11-15) were cut in the lanes' briefs and
never had ticket files, so the picker cannot see them; their record is their commits
(`git log --grep 'KK-27[0-9]'`; the lanes' merges `8d6c998a`, `92773276` and `957773f5`), and
their "Landed" lines, retired with the rest on 2026-11-15, are in `git show
6bace482:plan/STATUS.md` (KK-289, parked at first, landed on `main` as `a6f7cfaf`; KK-273 was
never used). The same holds for KK-292 to KK-295 (`0e3ee544`), KK-300 to KK-313 (`544da6f8`),
KK-320 to KK-324 (`91a694ee`) and KK-350 to KK-353 (the docs cleanup of 2026-11-15).

KK-246 to KK-249 each add one test to `crates/ui-egui/src/app/harness/tests.rs`, so `auto`
picks one of them a night (owns overlap). To run all four the same night, queue them as a
list (`overnight kitkraft KK-246+KK-247+KK-248+KK-249 4`); the merge then keeps every
ticket's test in that file and every ticket's row in the findings table by hand.

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
