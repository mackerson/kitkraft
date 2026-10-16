# Next sprint (M5, from 2026-10-15)

Which M5 tickets (`plan/execution-plan.md`, "M5") start now, which wait on the owner, and
which model does what. Dependencies: `docs/foothold/dependency-graph.md`. Status today:
`docs/foothold/gap-matrix.md`. Every hand-off ends with the AGENTS.md "Reporting" block.

## Agent roles

| Role | Model | Does |
| ---- | ----- | ---- |
| Writer | Opus | Owns the ticket in its worktree: code, tests, docs, the gate, the commit, the 40-line hand-off |
| Reviewer | Fable | Reviews each writer branch before the director merges it: clean-room rules, evidence behind every status cell, scope (no refactor, no Foothold content), and the six statuses kept apart |
| Triage | Haiku | First pass over bulk output: sorts differential lines, probe results, log lines and corpus query hits into classes for the writer; never sets a status and never commits |

## Starts now, in parallel

Lanes B and D started today (owner, 2026-10-15). The others listed here need no owner
action either. One worktree and one `CARGO_TARGET_DIR` per writer.

| Ticket | Lane | Writer | Reviewer | Triage | Notes |
| ------ | ---- | ------ | -------- | ------ | ----- |
| M5.B1 CK sources provenance | B (started) | Opus | Fable | Haiku: file-list comparison of the extraction against the zip | Fills the CK-CONTENT-RESOURCES row (ADR 0011 §4); nothing from the sources enters the repo |
| M5.B2 PEX differential | B (started) | Opus | Fable | Haiku: per-script diff classes | The harness can be built now; its CK-flags half waits on M5.B1's facts. Needs no owner step if the compiler's CK flags are read from the CK's own files, black box |
| M5.D1 generated support matrix | D (started) | Opus | Fable | - | `xtask` and `docs/support-evidence.toml`; seed it from `docs/foothold/gap-matrix.md` |
| M5.D2 `LCTN` typed write | D (started) | Opus | Fable | Haiku: corpus clone-sample mismatches | First with M5.D5: few opaque members (`kit schema`: LCTN 2, FACT 0) |
| M5.D5 `FACT` typed write | D (started) | Opus | Fable | - | Rank group at `reference` today: raise it or refuse |
| M5.D3 `TERM` typed write | D (started, after D2's handler changes land) | Opus | Fable | Haiku: corpus clone-sample mismatches | 19 `reference` members; `WBDT` opaque |
| M5.D4 `PKIN` typed write | D (started, after D2) | Opus | Fable | - | Component bodies stay raw; the `VNAM` misread is the first fix |
| M5.A1 CK session script | A | Opus | Fable | - | Write-only (docs): F29, F30, then F21, F20, F06. Feeds the owner's M5.O1 |
| M5.A2 `cargo xtask oracle ck` | A | Opus | Fable | - | Synthetic fixture folders only until M5.O1 |
| M4.T2 `probes.toml` | C | Opus | Fable | - | Depends only on M4.E6 (done) |
| M4.T3 + M5.C1, game-free parts | C | Opus | Fable | - | Preflight refusals, backups and restore-only rollback, authorized-path test, timeouts, `inconclusive`, probe runner against a fake endpoint, ledger writer. The launch command waits for M4.T1 |
| M5.F1 engine limitation register | F | Opus | Fable | Haiku: harvest candidate rows from `docs/gaps.md` and "Not in M4" | Evidence-cited rows only |
| M5.F3 version and DLC matrix | F | Opus | Fable | - | Settles the owner's DLC mapping (ADR 0011 §3) from the install manifests |
| M5.F4 ledger transcription | F | Opus | Fable | - | Two entries (M3.O5, M4.O2) from STATUS and the build docs |
| M5.F2 gap matrix upkeep | F | Opus | Fable | - | After every M5 merge, until M5.D1 generates the matrix |
| M5.H1 hostile-encounter research | H | Opus | Fable | Haiku: first pass over `kit query` and `papyrus events` hits | Research only; no hook, no Foothold attack design |

## Waits on the owner

| Ticket | Waits on | Then |
| ------ | -------- | ---- |
| M5.A3 golden run (path a) | **M5.O1**: the owner-assisted CK sitting for F29 and F30 | Opus writer, Fable reviewer, Haiku triages the `unexplained` lines |
| M5.A5 remaining fixtures | Later M5.O1 sittings (F20..F28) | As M5.A3 |
| M4.T1 session facts, V0 | **M4.O7**: console plugins installed into `KitKraft Dev`, INI switches, `KitKraftBase` | Opus; the first game launch by an agent, under ADR 0010 Amendment 1 |
| M4.T3 + M5.C1 completion | M4.T1; **M5.O2** at the merge (blocks only on a license outside MIT or Apache-2.0) | - |
| M5.C2 first unattended save/reload (path b) | M4.T3 + M5.C1, M4.O7 | Opus writer; Haiku triages probe output; Fable reviews the ledger entry against the evidence rule |
| M5.B3 scripted activator in game | **M4.O4** (owner) or M4.T6 (lane C) | - |
| FTH-EXP-01..12 | M5.C2, and the lane D and A tickets each row names | Not this sprint |
| M5.A4 unattended CK | M5.A3 | Not this sprint |
| M5.H2 encounter fixture | M5.H1, M5.C2, M5.D7 | Backlog |

## Owner actions this sprint

In `plan/STATUS.md` "Owner next actions" (rows 10, 12 and 13): M4.O7 (path b), M5.O1 (path
a), M5.O2 (at the M4.T3 merge). The M4.O2 persistence row and the M4.O4 session stay open
for the owner (row 2 and M5.O3); lane C may close them first.

## Merge order

The director merges as branches pass review: lane D's handler changes first (D2 before D3
and D4, since they share `handlers/newform.rs` and `edit/`); `defs/` changes go through BX
(`cargo xtask codegen`, parity) one family at a time; shared files (`plan/STATUS.md`,
`plan/execution-plan.md`, `docs/source-registry.md`) in small separate commits.
