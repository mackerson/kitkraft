# Tickets for the local agent (ADR 0014)

Each ticket is one file, `<ID>.toml`, run by the overnight harness as one unattended
supervised-solo run (`docs/local-agent.md`). The agent sees it rendered as `TICKET.md`.

## Format

```toml
id = "KK-001"                 # also the branch name: ticket/KK-001
title = "One line"
timeout_min = 45              # per grading command (default 45)
gate = "cargo xtask ci"       # default; the Linux gate
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

## Queue

`overnight kitkraft KK-002+KK-003+KK-004 8` runs several tickets back to back in one budget
(`docs/local-agent.md`). Each starts from `main`, so queued tickets must not depend on each
other, and their `owns` should not overlap. The STATUS line and a shared doc are the
exceptions, merged by hand.

| Ticket | Title | Source | State |
| ------ | ----- | ------ | ----- |
| [KK-001](KK-001.toml) | The gate passes on Linux | the 2026-10-23 trial: 20 tests assume Windows | merged `34e4a55` |
| [KK-002](KK-002.toml) | Typed-write tests for ACTI, KYWD, TMLM, QUST and WRLD | next-sprint item 2 (evidence entries) | run 1: 4/8, no commits; run 2 (2026-10-25) failed to start (harness bug); requeued from `kk/KK-002-wip` |
| [KK-003](KK-003.toml) | MCP progress notifications | next-sprint item 6 (M6.P2b rest) | merged `472871e` |
| [KK-004](KK-004.toml) | Clone-sample corpus tests for LCTN and TERM | next-sprint item 2 (M5.D2, M5.D3 rest) | merged `1f0940c` |
| [KK-005](KK-005.toml) | validate warns when a location's added persistent reference is temporary | M5.D2 rest | merged `f4acf8e` |
| [KK-006](KK-006.toml) | Type ARMA and name the biped slot bits | apparel requests 1 and 2 | run 1: 0/6 (scope bug in the ticket); run 2 (2026-10-25) failed to start (harness bug); requeued from `kk/KK-006-wip` |
| [KK-007](KK-007.toml) | Decode REFL losslessly; LMSW swap entries in `inspect --full` | apparel request 7 | merged `7313fbf` |
| [KK-008](KK-008.toml) | Inventory definition for LMSW, the layered material swap | apparel request 7 (`everyday-outfits`) | queued after KK-006 |
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

On 2026-10-25 KK-002 and KK-006 failed to start: the harness did not resolve their WIP base
branch (fixed in the harness). Both are requeued unchanged, still continuing from their WIP
branches; KK-008 still waits for KK-006 to merge. The same night KK-005 (run 2) and KK-007
landed.

KK-002 and KK-004 both add a section to `docs/schema/support-matrix.md`, and KK-004 and
KK-006 both may add a row to `docs/gaps.md`. Merge one, then resolve the other by hand.
Schema tickets read the pinned xEdit file at `/opt/xedit-ref` (`kk-setup` binds
`<checkout>\..\tools\xedit-ref` when it exists).

KK-007 and KK-008 are the read half of apparel request 7 and do not depend on each other
(KK-007 reads `REFL` bytes whether or not `LMSW` has a definition). KK-008 shares schema
and generated-code paths with KK-006 (ARMA), so queue it after KK-006 has merged. Writing an `LMSW` (`set`
on its swap paths, or `new` from entries) is cut after KK-007 lands, on its decoder.

Still to cut, from `docs/foothold/next-sprint.md`: the location and terminal recipes (item 2),
the Cell View's engine-facing tests (item 5). Window, CK-sitting and design-heavy work (M5.D1's
generated gap matrix) stays with Claude or the owner.

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
