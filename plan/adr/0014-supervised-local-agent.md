# ADR 0014: Development by a supervised local agent

- Status: **Accepted by the owner, 2026-10-23.** Owner's words: "it's too expensive to work on it
  the way we've been doing it for what it is, doubly so where I want to use my account tokens to
  work on our next project ... However, I'd like to keep this going, so: let's pivot." Supervisor
  choice: "Opus check-in".
- Supersedes, for routine work: the director pattern in AGENTS.md "Parallel agents" (Opus writers
  in worktrees, Fable reviewers, a Claude director merging). That pattern stays available for an
  owner who asks for it; it is no longer the default.

## Context

From 2026-10-14 to 2026-10-16 KitKraft was built by a Claude director running Opus writer lanes
and Fable reviewers. It worked (654 commits, every merge behind a green gate), and it cost about
3.8 billion tokens, 92% of them Opus writers (`docs/analytics/2026-10-16-project-analytics.md`).

The owner has a local model stack (`F:\AI\Scripts`, see its `README-qwen.md` and
`overnight\README.md`): Qwen3.8-27B on vLLM in WSL, driving Claude Code (`claude-local`) at no
API cost, and a sandboxed overnight harness that grades unattended runs against a hidden suite.
On the same task and seed (`overnight\runs\DIRECTOR-AB-ANALYSIS.md`):

| Setup | Score | Paid usage |
| ----- | ----- | ---------- |
| local agent alone, 3.5 h | 89.7% | none |
| Fable director over local workers | 13.1% | director and reviewers |
| Opus director over local workers | 44.5% | director and reviewers |
| local agent, read-only Opus check-in every 15 min (run `20261020-1234`) | 94.1% in 45 min | about 0.6M Opus cache tokens, almost no output |

Directors over local workers lost because they sliced work horizontally, split the GPU across
lanes, and restarted workers fresh so code was rewritten. The supervised solo agent kept one
continuous session on one codebase and took short written advice.

## Decision

1. **The author is the local agent.** One ticket per run, one continuous Claude Code session on
   `qwen3.8-27b`, inside the overnight harness's sandbox (user `agent`, bubblewrap, loopback-only
   network), on a clone of this repository on WSL's own disk.
2. **The supervisor is a read-only Opus check-in** every 15 to 60 minutes (the harness's
   `--checkin-min`): it reads the workspace and `git log`/`git diff`, writes up to ~250 words of
   prioritised notes, and the runner feeds them into the agent's next prompt. It never writes
   code, never sees the grader's results, and runs on the owner's subscription.
3. **The grader is KitKraft's own gate.** `cargo xtask ci` (offline) plus the ticket's
   acceptance commands, run on a snapshot at each checkpoint, outside the agent's sandbox. A
   ticket is done when every acceptance command and the gate pass.
4. **Tickets live in `plan/tickets/`**, one file each, written before the run: scope (paths the
   agent owns), the brief, acceptance commands, and what the agent must not read. They are cut
   as vertical slices (lesson 1 of the director A/B). The queue is `plan/tickets/README.md`.
5. **Merging stays behind a check.** A run ends with the agent's commits as a git bundle in the
   run folder. Nothing reaches `main` without a green gate on Windows and a review: a short
   Claude review when the owner asks, or the owner's own. The local agent never pushes.
6. **Owner-gated work is unchanged.** CK sittings, in-game sessions, Nexus installs and license
   rulings stay in `plan/STATUS.md` "Owner next actions".

## Consequences

- Paid usage drops to the check-ins and the occasional review: roughly 1M cache tokens per
  overnight run instead of hundreds of millions per wave.
- Throughput drops. A 27B model on a 136k context, compiling Rust, will stall on large or
  cross-cutting tickets; tickets must be small, owned-path, testable, and their acceptance
  commands must be runnable headless (no window screenshots, no game, no CK).
- The GPU is busy during a run, so runs and Starfield sessions do not overlap.
- The agent cannot reach the network: Rust dependencies are fetched once into the agent's
  `CARGO_HOME` by the setup script, and builds run with `CARGO_NET_OFFLINE=true`.
- Tests that read the game's Data folder see it through a read-only bind; on Linux the
  Windows-only paths (registry, `tasklist`, MO2 detection) must degrade to refusals or skips,
  which the trial build of 2026-10-23 checks (`docs/local-agent.md`).
- AGENTS.md "Start every session" still applies to the local agent except that it reads its
  ticket instead of `plan/STATUS.md` and `plan/execution-plan.md` (about 37,000 and 39,000
  words, too large for its context).
