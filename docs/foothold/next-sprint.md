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

Lanes B and D were started on 2026-10-15 before this plan existed, on briefs that match
M5.B1 and M5.B2 and M5.D1 to M5.D3 in substance. Their branches carry no ticket ids; the
director reconciles each merge to the ticket ids below (and records which brief became
which ticket in the merge commit). The other tickets listed here need no owner action.
One worktree and one `CARGO_TARGET_DIR` per writer.

| Ticket | Lane | Writer | Reviewer | Triage | Notes |
| ------ | ---- | ------ | -------- | ------ | ----- |
| M5.B1 CK sources provenance | B (running, pre-plan brief) | Opus | Fable | Haiku: file-list comparison of the extraction against the zip | Fills the CK-CONTENT-RESOURCES row (ADR 0011 §4); nothing from the sources enters the repo |
| M5.B2 PEX differential | B (running, pre-plan brief) | Opus | Fable | Haiku: per-script diff classes | The harness can be built now; its CK-flags half waits on M5.B1's facts. Needs no owner step if the compiler's CK flags are read from the CK's own files, black box |
| M5.D1 generated support matrix | D (running, pre-plan brief) | Opus | Fable | - | `xtask/src/parity.rs` and `docs/support-evidence.toml`; seed it from `docs/foothold/gap-matrix.md`. Shares `xtask/src/main.rs` with lane A (merge order below) |
| M5.D2 `LCTN` typed write | D (running, pre-plan brief) | Opus | Fable | Haiku: corpus clone-sample mismatches | First with M5.D5: few opaque members (`kit schema`: LCTN 2, FACT 0); LCTN has 8 `reference` members, FACT 6 |
| M5.D3 `TERM` typed write | D (running, pre-plan brief; after D2's handler changes land) | Opus | Fable | Haiku: corpus clone-sample mismatches | 19 `reference` members; `WBDT` opaque |
| M5.D5 `FACT` typed write | D (not started) | Opus | Fable | - | Rank group at `reference` today: raise it or refuse |
| M5.D4 `PKIN` typed write | D (not started; after D2) | Opus | Fable | - | Component bodies stay raw; the `VNAM` misread is the first fix |
| M5.A1 CK session script | A | Opus | Fable | - | Write-only (docs): F29, F30, then F21, F20, F06. Not a prerequisite of M5.O1, which can run from STATUS row 12 now; A1 makes the ledger's session script and serves the later sittings (M5.A5) |
| M5.A2 `cargo xtask oracle ck` | A | Opus | Fable | - | Synthetic fixture folders only until M5.O1; afterwards reruns M5.A3's fixtures and replaces the hand classing. Rebases onto M5.D1 in `xtask/src/main.rs` |
| M4.T1 (a) MO2 command line | C | Opus | Fable | - | Needs neither the game nor M4.O7: `ModOrganizer.exe --help`, black box, nothing written into `C:\Modding\MO2-Starfield`. Facts (b)..(h) wait on M4.O7 |
| M4.T2 `probes.toml` | C | Opus | Fable | - | Depends only on M4.E6 (done); M5.D7 needs it |
| M4.T3 + M5.C1, game-free parts | C | Opus | Fable | - | Preflight refusals, backups and restore-only rollback, authorized-path test, timeouts, `inconclusive`, probe runner against a fake endpoint, ledger writer. The launch command waits for M4.T1 |
| M5.F4 ledger transcription | F (first) | Opus | Fable | - | Two entries (M3.O5, M4.O2) from STATUS and the build docs, with the plugins' hashes. Until it lands, the gap matrix writes those results `pass (owner report; ledger entry pending M5.F4)` |
| M5.F1 engine limitation register | F | Opus | Fable | Haiku: harvest candidate rows from `docs/gaps.md` and "Not in M4" | Evidence-cited rows only |
| M5.F3 version and DLC matrix | F | Opus | Fable | - | Settles the owner's DLC mapping (ADR 0011 §3) from the install manifests |
| M5.F5 Free Lanes and Terran Armada outpost changes | F | Opus | Fable | Haiku: first pass over `kit conflicts` and `kit diff` output | Addendum item 5: what `SFBGS050.esm` and `SFBGS00D.esm` override or add in the outpost and workshop families, read only, each row with its command; feeds FTH-EXP-09 |
| M5.F2 gap matrix upkeep | F | Opus | Fable | - | After every M5 merge, until M5.D1 generates the matrix |
| M5.H1 hostile-encounter research | H | Opus | Fable | Haiku: first pass over `kit query` and `papyrus events` hits | Research only; no hook, no Foothold attack design. Scope includes "Combat behavior, settlement defenses, damage, and recovery." and "Compatibility with vanilla outpost assaults and current DLC behavior." (addendum item 3) |

## Waits on the owner

| Ticket | Waits on | Then |
| ------ | -------- | ---- |
| M5.A3 golden run (path a) | **M5.O1** only: the owner-assisted CK sitting for F29 and F30, which can start now from STATUS row 12 | Opus writer with `cargo xtask observe`, `kit roundtrip --verify --decode`, `validate` and `kit diff`; Fable reviewer; Haiku triages the `unexplained` lines. M5.A2 reruns it later |
| M5.A5 remaining fixtures | **M4.O1**: the owner's CK sittings for F20..F28 (STATUS row 15); M5.A1, M5.A2 | As M5.A3 |
| M4.T1 (b)..(h) session facts, V0 | **M4.O7**: console plugins installed into `KitKraft Dev`, INI switches, `KitKraftBase` | Opus; the first game launch by an agent, under ADR 0010 Amendment 1 |
| M4.T3 + M5.C1 completion | M4.T1; **M5.O2** at the merge (blocks only on a license outside MIT or Apache-2.0) | - |
| M5.C2 first unattended save/reload (path b) | M4.T3 + M5.C1, M4.O7 | Opus writer; Haiku triages probe output; Fable reviews the ledger entry against the evidence rule |
| M5.B3 scripted activator in game | **M4.O4** (owner, STATUS row 14) or M4.T6 (lane C) | - |
| FTH-EXP-01..12 | M5.C2 **or M5.O4** (an owner-assisted sitting from the ticket's testplan), and the lane D, A and F tickets each row names | Not this sprint |
| M5.A4 unattended CK | M5.A3 | Not this sprint |
| M5.H2 encounter fixture | M5.H1, M5.C2, M5.D7 | Backlog |

## Owner actions this sprint

In `plan/STATUS.md` "Owner next actions" (rows 10, 12 to 15): M4.O7 (path b), M5.O1 (path
a; can start now), M5.O2 (at the M4.T3 merge), M4.O4 (session 2 with the scripts recipe;
M5.B3 and M5.O3 wait on it unless lane C's M4.T6 closes it first) and M4.O1 (the F20..F28
CK sittings; M5.A5 waits on them). The M4.O2 persistence row stays open for the owner
(row 2 and M5.O3); lane C may close it first.

## Merge order

The director merges as branches pass review: lane D's handler changes first (D2 before D3
and D4, since they share `handlers/newform.rs` and `edit/`); `defs/` changes go through BX
(`cargo xtask codegen`, parity) one family at a time; shared files (`plan/STATUS.md`,
`plan/execution-plan.md`, `docs/source-registry.md`) in small separate commits.

- `xtask`: lane D owns `xtask/src/parity.rs` for M5.D1 and lane A owns
  `xtask/src/oracle.rs` and `xtask/src/evidence.rs` for M5.A2; they share only the
  subcommand dispatch in `xtask/src/main.rs`. M5.D1 merges first (it is running); M5.A2
  rebases onto it and keeps its dispatch change to its own line.
- Lanes B and D's pre-plan branches: each merge commit names the M5 ticket ids it closes
  (M5.B1, M5.B2, M5.D1 to M5.D3) and what of the ticket's "Done when" is still open.
