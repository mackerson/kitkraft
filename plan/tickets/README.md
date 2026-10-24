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
| [KK-002](KK-002.toml) | Typed-write tests for ACTI, KYWD, TMLM, QUST and WRLD | next-sprint item 2 (evidence entries) | run 1: 4/8, no commits; run 2 continues from `kk/KK-002-wip` |
| [KK-003](KK-003.toml) | MCP progress notifications | next-sprint item 6 (M6.P2b rest) | merged `472871e` |
| [KK-004](KK-004.toml) | Clone-sample corpus tests for LCTN and TERM | next-sprint item 2 (M5.D2, M5.D3 rest) | merged `1f0940c` |
| [KK-005](KK-005.toml) | validate warns when a location's added persistent reference is temporary | M5.D2 rest | run 1: 2/7, 3 lines (context limits); requeue |
| [KK-006](KK-006.toml) | Type ARMA and name the biped slot bits | apparel requests 1 and 2 | run 1: 0/6 (scope bug in the ticket); run 2 continues from `kk/KK-006-wip` |
| [KK-007](KK-007.toml) | Decode REFL losslessly; LMSW swap entries in `inspect --full` | apparel request 7 | queued |
| [KK-008](KK-008.toml) | Inventory definition for LMSW, the layered material swap | apparel request 7 (`everyday-outfits`) | queued after KK-006 |

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
