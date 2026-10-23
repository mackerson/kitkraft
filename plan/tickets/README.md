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

## Queue

Run in this order; each starts from `main` after the previous one is merged.

| Ticket | Title | Source |
| ------ | ----- | ------ |
| [KK-001](KK-001.toml) | The gate passes on Linux | the 2026-10-23 trial: 20 tests assume Windows |

Next candidates, from `docs/foothold/next-sprint.md` "Next, in order", to be cut into tickets
once KK-001 is merged: the location and terminal recipes (item 2), the evidence entries for
ACTI/KYWD/TMLM/QUST/WRLD (item 2), MCP progress notifications (item 6), the Cell View's
engine-facing tests (item 5). Window and CK-sitting work stays with Claude or the owner.
