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
| [KK-002](KK-002.toml) | Typed-write tests for ACTI, KYWD, TMLM, QUST and WRLD | next-sprint item 2 (evidence entries) | run 1: 4/8, no commits; run 2 (2026-10-25) failed to start (harness bug); run 3 (`kk-KK-002-20261025-1330`, the old acceptance): 4/8; it ignored both WRLD tests with a reason and moved QUST to the `PlanetContentProperties` layout, but QUST still fails and no evidence entries were written. Requeued from `kk/KK-002-wip` (`56ee2e1`) with the fixed acceptance and a fourth-run brief: ignore QUST, drop the scratch probe test, write the evidence. Follow-ups to cut: QUST's slot misalignment (engine), `new` for WRLD |
| [KK-003](KK-003.toml) | MCP progress notifications | next-sprint item 6 (M6.P2b rest) | merged `472871e` |
| [KK-004](KK-004.toml) | Clone-sample corpus tests for LCTN and TERM | next-sprint item 2 (M5.D2, M5.D3 rest) | merged `1f0940c` |
| [KK-005](KK-005.toml) | validate warns when a location's added persistent reference is temporary | M5.D2 rest | merged `f4acf8e` |
| [KK-006](KK-006.toml) | Type ARMA and name the biped slot bits | apparel requests 1 and 2 | merged `e7bc2d2` |
| [KK-007](KK-007.toml) | Decode REFL losslessly; LMSW swap entries in `inspect --full` | apparel request 7 | merged `7313fbf` |
| [KK-008](KK-008.toml) | Inventory definition for LMSW, the layered material swap | apparel request 7 (`everyday-outfits`) | queued (KK-006 merged) |
| [KK-010](KK-010.toml) | testplan: add, equip and look at new and overridden armor | apparel request 5 | merged `33307be` |
| [KK-011](KK-011.toml) | Corpus survey: which object mod properties armor carries | apparel request 3 (second half) | queued |
| [KK-012](KK-012.toml) | Recipe: a location of your own (`docs/recipes/location.md`) | M5.D2 rest | queued |
| [KK-013](KK-013.toml) | Recipe: a terminal with its own menus (`docs/recipes/terminal.md`) | M5.D3 rest | queued |
| [KK-014](KK-014.toml) | Clone-sample corpus tests for PKIN and FACT | M5.D4, M5.D5 rest | queued |
| [KK-015](KK-015.toml) | Ownership recipe, new-faction variant | M5.D5 rest | queued |
| [KK-016](KK-016.toml) | `cargo xtask parity` generates the gap matrix headline and checks the family table | M5.D1 rest | after KK-002 |
| [KK-017](KK-017.toml) | Typed-write tests for CONT, MISC, FLST and LVLI | gap matrix "untested" rows | after KK-002 |
| [KK-018](KK-018.toml) | validate warns when a model file a record names is nowhere to be found | apparel requests 4 and 8 (asset checks) | after KK-005 |
| [KK-019](KK-019.toml) | Write LMSW material swaps: `swap_overrides` on `set` and `clone` | apparel request 7 (write half) | after KK-007, KK-008 |
| [KK-020](KK-020.toml) | Recipe: a clothing colourway (build spec `swap_overrides`) | apparel request 7 | after KK-019 |
| [KK-021](KK-021.toml) | Typed-write tests for ARMO, ARMA and OTFT | apparel requests 1 and 2 | after KK-006, KK-017 |
| [KK-022](KK-022.toml) | Inventory definitions for CURV, REGN, AVMD, IPCT and DFOB | record parity (types with no definition) | after KK-008 |
| [KK-023](KK-023.toml) | Type FLOR (flora) from the pinned xEdit file | record parity (lstring-only FLOR) | after KK-022 |
| [KK-030](KK-030.toml) | Engine test support: game-session fakes that front ends can drive | M6.G4 to G6 against M4.T3's fakes (next-sprint item 3) | queued |
| [KK-031](KK-031.toml) | The workbench gate permits `game.session` (start) and `game.stop`; `game.status` allowed | M6.G4, first half | after KK-030 |
| [KK-032](KK-032.toml) | The Cockpit launches a game session and follows it by polling `game.status` | M6.G4, second half | after KK-031 |
| [KK-033](KK-033.toml) | The Cockpit shows a session's probe rows and screenshots | M6.G5, first half | after KK-032 |
| [KK-034](KK-034.toml) | `game.probe` from the Cockpit: re-run, screenshot, send a row to the Ledger | M6.G5 second half, M6.G6 wiring | after KK-033 and KK-035 |
| [KK-035](KK-035.toml) | The Ledger form is prefilled from a probe row left inconclusive or manual | M6.G6 | queued |
| [KK-036](KK-036.toml) | The Cell View window over `cell` | M7.W2 (next-sprint item 5) | queued |
| [KK-037](KK-037.toml) | Cell View and Render Window in step: open a cell in the viewport, one selection | M7.R2 selection sync | after KK-036 |
| [KK-038](KK-038.toml) | Table columns resize by dragging the header's edge | M6.V5 rest | queued |
| [KK-039](KK-039.toml) | Raw JSON held in one buffer and sliced by the visible range | M6.P5a rest | queued |
| [KK-040](KK-040.toml) | The workbench loads the last profile at start, from the index cache | M6.P3a rest | queued |
| [KK-041](KK-041.toml) | Measure whether opt-level 2 for dependencies makes debug runs usable | M6.P6a rest | queued |
| [KK-042](KK-042.toml) | `game.session` reports progress: preflight, each phase, each probe row | M6.G4 ("progress events for its own steps") | after KK-030 |
| [KK-043](KK-043.toml) | MCP: a game session started over MCP is followed and stopped across calls | CLI/MCP/UI parity for M4.T3's commands | after KK-030 |
| [KK-050](KK-050.toml) | Draft the KK-002 follow-ups: QUST's condition slots, `new` for a WRLD (2 drafts) | KK-002's follow-ups to cut | queued |
| [KK-051](KK-051.toml) | Draft three apparel tickets: ARMO addons in `inspect`, LMSW `.mat` paths, armor world models | "next in engine and data" (below) | queued |
| [KK-052](KK-052.toml) | Draft two schema tickets: inventory definitions for MRPH, CNDF, VTYP, MTPT, ASPC; type AVIF | "next in engine and data" (below) | queued |
| [KK-090](KK-090.toml) | Schema tickets merge cleanly: the defs test derives its record list, shared lists one entry per line | the generated-file overlap (above, "Generated tickets") | queued (priority 2) |
| [KK-100](KK-100.toml) | Inventory definitions for SFTR, TRNS, PDCL, EFSQ and STMP | generated by plan/tickets/generate.py inventory | queued |
| [KK-101](KK-101.toml) | Inventory definitions for ATMO, KSSM, BMOD, FFKW and LTEX | generated by plan/tickets/generate.py inventory | queued |
| [KK-102](KK-102.toml) | Inventory definitions for LVLB, AMBS, IPDS, IMGS and MUST | generated by plan/tickets/generate.py inventory | queued |
| [KK-103](KK-103.toml) | Inventory definitions for CAMS, MATT, IMAD, AACT and SPCH | generated by plan/tickets/generate.py inventory | queued |
| [KK-104](KK-104.toml) | Inventory definitions for GRAS, WTHS, STND and CSTY | generated by plan/tickets/generate.py inventory | queued |
| [KK-105](KK-105.toml) | Inventory definitions for LGDI, DLBR, FSTP, MUSC and STBH | generated by plan/tickets/generate.py inventory | queued |
| [KK-106](KK-106.toml) | Inventory definitions for GCVR, FOGV, ADDN, EFSH and BMMO | generated by plan/tickets/generate.py inventory | queued |
| [KK-107](KK-107.toml) | Inventory definitions for COLL, SFPC, CLMT, PTST, VOLI, MOVT, REVB and CUR3 | generated by plan/tickets/generate.py inventory | queued |
| [KK-108](KK-108.toml) | Inventory definitions for SECH, STAG, LENS, EQUP, AAPD, AOPS, CLDF and AOPF | generated by plan/tickets/generate.py inventory | queued |
| [KK-109](KK-109.toml) | Inventory definitions for LVSC, FSTS, IDLM, SCOL, BNDS, TODD, CLFM and AORU | generated by plan/tickets/generate.py inventory | queued |
| [KK-110](KK-110.toml) | Inventory definitions for PSDC, GBFT, NAVI, AAMD, LGTM, WBAR, FORC and GPOG | generated by plan/tickets/generate.py inventory | queued |
| [KK-111](KK-111.toml) | Inventory definitions for DEBR, OSWP, WTHR, PERS, SDLT, DOBJ, GWED and MAAM | generated by plan/tickets/generate.py inventory | queued |
| [KK-112](KK-112.toml) | Inventory definitions for NOCM, OVIS, SCCO and SPGD | generated by plan/tickets/generate.py inventory | queued |
| [KK-200](KK-200.toml) | Type ALCH (Ingestible) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-201](KK-201.toml) | Type BOOK (Book) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-202](KK-202.toml) | Type DOOR (Door) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-203](KK-203.toml) | Type WATR (Water) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-204](KK-204.toml) | Type EXPL (Explosion) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-205](KK-205.toml) | Type HAZD (Hazard) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-206](KK-206.toml) | Type LIGH (Light) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-207](KK-207.toml) | Type KEYM (Key) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-208](KK-208.toml) | Type HDPT (HeadPart) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-209](KK-209.toml) | Type ENCH (Enchantment) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-210](KK-210.toml) | Type ARTO (ArtObject) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-211](KK-211.toml) | Type TXST (TextureSet) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-212](KK-212.toml) | Type BPTD (BodyPartData) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-213](KK-213.toml) | Type PGRE (PlacedProjectile) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-214](KK-214.toml) | Type PHZD (PlacedHazard) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-215](KK-215.toml) | Type IRES (Resource) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-216](KK-216.toml) | Type MESG (Message) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-217](KK-217.toml) | Type GPOF (GameplayOption) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-218](KK-218.toml) | Type SOUN and WKMF from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |
| [KK-219](KK-219.toml) | Type RFGP (ReferenceGroup) from the pinned xEdit file, corpus-checked | generated by plan/tickets/generate.py depth | queued |

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
- schema: KK-006 -> KK-008 -> KK-022 -> KK-023 (all regenerate `crates/esm/src/generated/` and
  the golden seed; never two in one night).

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
