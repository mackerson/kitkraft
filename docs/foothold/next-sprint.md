# Next sprint (from the 2026-10-16 pause)

What landed on 2026-10-16, what a resuming session starts, in which order, what waits on
the owner, and which director decisions wait for the owner's confirmation. Written by the
reconciliation lane against `main` at `81272e2` (after fix round W3). Ticket text and
"done when": `plan/execution-plan.md` (M4 lane T, M5, M6, M7). Dependencies:
`docs/foothold/dependency-graph.md`. Status per capability: `docs/foothold/gap-matrix.md`
(checked at `81272e2`). Every hand-off ends with the AGENTS.md "Reporting" block.

## Agent roles

| Role | Model | Does |
| ---- | ----- | ---- |
| Writer | Opus | Owns the ticket in its worktree: code, tests, docs, the gate, the commit, the 40-line hand-off |
| Reviewer | Fable | Reviews each writer branch before the director merges it: clean-room rules, evidence behind every status cell, scope (no refactor, no Foothold content), and the six statuses kept apart |
| Triage | Haiku | First pass over bulk output: sorts differential lines, probe results, log lines and corpus query hits into classes for the writer; never sets a status and never commits |

## Landed on 2026-10-16

By ticket id, in merge order. Each lane's evidence is its "Landed" line in
`plan/STATUS.md` and its ticket row in `plan/execution-plan.md`. "Partial" names what is
open; nothing below was run in the game or the CK.

| Lane | Done | Partial, or built but not run | Open from it |
| ---- | ---- | ----------------------------- | ------------ |
| H | - | M5.H1 research (result hypothesis) | - |
| F | M5.F1, M5.F3, M5.F4, M5.F5 | M5.F2 (upkeep: checked at `5d03d01`, then at `81272e2` by this lane) | M5.F2 continues until M5.D1 generates the matrix |
| P, engine side | M6.P2b, M6.P6b | M6.P6a (engine side) | MCP progress notifications (P2b) |
| A + B | M5.A1 (F29/F30 script), M5.A2 (`oracle ck`, synthetic fixtures), M5.B1, M5.B2 (50 of 50 equivalent) | M5.A4 inconclusive (method pending the owner, row 17); M7.S4 prep memo | F21, F20, F06 session scripts (M5.A1) |
| V, rounds 1 to 3 | M6.V1, M6.V2, M6.V3, M6.V4, M6.V6; M6.O1 recorded | M6.V5 | Column resize by drag (V5) |
| C (runtime) | M4.T2, M4.T9 | M4.T3 + M5.C1 built and tested against fakes only; M4.T1 (a) not-run (`game-20261016-T1`) | M4.T1, then live sessions (row 10) |
| D, round 3 | M5.D6, M6.P3b, M6.P5b, M7.S3, M7.S6, M7.S7 | - | - |
| G | M6.G1, M6.G2, M6.G3, M6.G7, M6.G8 | M7.E1 (Amendment 4 drafted, proposed) | M6.G4 to M6.G6 |
| R, round 1 | - | M7.R1 (ADR 0013 proposed) | V1 measured with the window driven |
| D7 / H2 | - | M5.D7 (mini-site; game rows not-run), M5.H2 first half (prototype) | M5.H2 second half (waits on M5.C2) |
| E | - | FTH-EXP-01 and FTH-EXP-03 agent halves | their game and CK rows |
| T, round 2 | - | M4.T10 (stages 1 to 6 pass, the session refused) | the "done when" (M4.T1, row 10) |
| P, window side | M6.P2a, M6.P4, M6.P5a, M6.P7 | M6.P1, M6.P3a (no start-up load) | P1 rows L5, I1, I3, I4 (window), T4, U1; P3a start-up load; P5a raw JSON by visible range |
| D, round 4 | Typed-writable COBJ, STAT, REFR, ACHR, CELL; typed-readable WEAP, CONT (M5.D4/D5 follow-ups) | - | - |
| M | M7.M4, M7.W5 | M7.M0, M7.M1, M7.W3 (every row `built, unobserved`) | the CK's order and labels (M7.K3, M7.K4) |
| R, round 2 | - | M7.R2, M7.R3 (`OBND` variant) | Cell View sync (M7.W2); NIF bounds (M7.O5) |
| D, round 5 | M5.D8, M5.D9, M5.D10, M5.D11 (which also does M7.S6b) | - | - |
| Fix rounds | W1 (wave-1 review), X and P (dist, install), W2 (confirmations bound to their request, viewport GL objects released), W3 (the seven wave-3 items, `f758223` to `a8469e7`, marked in `34299f6`) | - | - |

From 2026-10-15 and still partial: M5.D1 (`kit coverage --matrix` and
`docs/schema/evidence.toml` exist; `cargo xtask parity --check` against this folder's
matrix does not), M5.D2 to M5.D5 (typed write and dangling-link tests, `24ac42d`; the
recipes `docs/recipes/location.md` and `terminal.md` and M5.D2's corpus clone-sample test
are not written).

## Next, in order

Agent-executable, no owner step. One worktree and one `CARGO_TARGET_DIR` per writer;
items 2 to 6 can run in parallel once item 1 is merged (they touch different files, see
"Merge order").

| # | Ticket(s) | Lane | Why now | Done when |
| - | --------- | ---- | ------- | --------- |
| 1 | Fable review of this reconciliation (`plan/STATUS.md` "Pause", section 2) | review | The planning documents are the resuming session's map; the review checks every cell against its evidence | The review's findings applied; STATUS "Pause" section 2 closed |
| 2 | M5.D1 rest (`cargo xtask parity --check` generates the family table of `gap-matrix.md` and checks it); `docs/schema/evidence.toml` entries for the recipe-backed writes (ACTI, KYWD, TMLM, QUST, WRLD); M5.D2 and M5.D3 rest (recipes `location.md`, `terminal.md`, the clone-sample corpus test) | D | Closes the agent side of M5's exit row "typed-writable with reference integrity" and ends the hand upkeep of this matrix (M5.F2) | As the ticket rows; MX and the gap matrix agree on typed-writable |
| 3 | M6.G4, M6.G5, M6.G6 against M4.T3's fakes | G | `game.session --start`, `game.status`, `game.probe` and `game.stop` are in the catalog; the shell-model tests their rows ask for run against the fake endpoint and fixture instance | Gate tests and shell-model tests; the live run (M6.O4, M4.T4 from the window) stays with the owner and row 10 |
| 4 | M7.K2 (interface sitting scripts 1 and 2) and M5.A1's F21, F20, F06 scripts | K, A | Docs only. With them the owner can hold one combined sitting for rows 12 and 15 and M7.O2/M7.O3 | The scripts in `docs/ck-observations.md` and `docs/ck-sessions/`; each step names the parity rows or fixtures it fills |
| 5 | M7.W2 Cell View, then M7.R2's selection sync | W, R | `cell` exists (M7.S3); the Render Window waits on it for its list sync | As the ticket rows; rows `built, unobserved` until a sitting |
| 6 | Leftovers: M6.V5 column resize; M6.P1's open rows; M6.P3a start-up load; M6.P5a raw JSON by visible range; M6.P6a opt-level measurement; M6.P2b MCP progress notifications; M7.R1's V1 with the window driven | V, P, R, engine core | Small, measured, no owner step | Before-and-after rows in `docs/design/workbench-performance.md` where the ticket asks for them |

Not next (blocked): M7.W1, M7.M2, M7.M3, M7.M5, M7.S1, M7.S2, M7.W4 (wait on M7.K3 or
M7.K4, so on the sittings); M7.E2 to M7.E6 (M7.O4); M7.R3's NIF variant, M7.R4, M7.R5,
M7.S4, M7.S5 (M7.O5); M4.T4 to M4.T8, M5.C2, M5.B3 and every FTH-EXP game row (row 10 or
an owner sitting); M5.A3, M5.A5 (rows 12 and 15). Lane X stays DEFER.

## Waits on the owner

The rows of `plan/STATUS.md` "Owner next actions" that gate tickets, and M7's two
sittings. In-game work only on `KitKraft Dev` with a disposable save.

| Owner row | Action | What waits on it |
| --------- | ------ | ---------------- |
| 10 (M4.O7) | Install the Console API, Console Command Runner Updated (and optionally Console Output To File) into `KitKraft Dev`; the INI switches (turn on the profile's own INI files first: `LocalSettings=false` today, ledger `game-20261016-T1`); the base save `KitKraftBase`. Also, with MO2 closed, `ModOrganizer.exe --help` for M4.T1 (a) | M4.T1 (a) to (h), so a live M4.T3 session; M4.T4 to M4.T7, M5.C2 (path b), M4.T10's "done when", the live half of M6.G4 and M6.O4, M5.H2's second half, every FTH-EXP game and save/reload row |
| 12 (M5.O1) | CK sitting 1: F29 and F30 (`docs/ck-sessions/f29-f30.md`) | M5.A3 (path a), Gate K0's CK item, every CK-diff cell, the M5.A4 experiment |
| 14 (M4.O4) | In-game session 2, template O4, the scripts recipe in game (O4-07) | M5.B3 (unless lane C's M4.T6 closes O4-07 first), Gate K0's scripted-activator item, M5.O3 |
| 15 (M4.O1) | CK sittings for F20 to F28, best after row 12 | M5.A5, so FTH-EXP-01, -04, -08 and -10's CK twins; M4.A1..A6's rules raised to `ck-diff` |
| 17 | Rule on a static strings scan of `CreationKit.exe` | M5.A4 (unattended CK route); until then `docs/ck-observations.md` "M5.A4" is not cited |
| M7.O2, M7.O3 | CK interface sittings 1 (menus, windows, camera) and 2 (edit dialogs of eight types), combinable with rows 12 and 15 once M7.K2's scripts exist | M7.K3, M7.K4, so every `built, unobserved` row reaching `observed`, the CK menu order, M7.M2, M7.M3, M7.M5, M7.S1, M7.S2, M7.W1, M7.W4 |

Also the owner's, gating less: M7.O4 (ADR 0007 Amendment 4, the editor write scope; M7.E2
to M7.E6), M7.O5 (NIF sources; M7.S4 onward), M6.O2 (fonts), row 7 (BSL-1.0), row 2's
persistence row O2-08, row 13 (review `crates/game`'s dependency list: it adds no
third-party crate, ADR 0010 "Dependency list"), and rows 5 and 11 (folder rename, install).

## Director decisions pending the owner's confirmation

Taken during the 2026-10-16 run so lanes could proceed; each stands until the owner
overturns it (`plan/STATUS.md` "Pause", section 3).

| Decision | Recorded in | If overturned |
| -------- | ----------- | ------------- |
| ADR 0013: `unsafe` in exactly one file (`crates/ui-egui/src/viewport/gl.rs`) for the GL viewport; lint `deny`, `cargo xtask layers` fails on `unsafe` elsewhere | `plan/adr/0013-scoped-unsafe-for-the-gl-viewport.md` (its status line still reads "pending the director's confirmation at the merge"; the director merged it and lists it as pending the owner) | M7.R1's GL path is reverted; the Render Window has no viewport |
| ADR 0012 Amendment 1: the parity status `built, unobserved`, below `observed`, never counted as parity | `plan/adr/0012-ck-interface-clean-room.md`, `docs/ck-parity.md` (13 rows) | The 13 rows go back to `unresearched` (the elements stay, unrated) |
| M7.M0: CK-style floating `egui::Window`s, no dock crate, docks only if a sitting shows them | `docs/workbench.md` "CK-style windows" | M7.M0 is redone with a docking crate listed in ADR 0007 |
| M7.R3 draws `OBND` boxes first; the NIF-derived bounds wait on M7.O5 | `plan/execution-plan.md` M7.R3 | M7.R3 waits wholly on M7.O5 |
| REFR rotation `Rx(-x)Ry(-y)Rz(-z)`, from corpus box-corner alignment in three cells, unobserved against the CK | `crates/ui-egui/src/viewport/scene.rs` (`ref_rotation`), `docs/ck-parity.md` | Re-pinned from a CK sitting (M7.O2) |
| M6.O2: Windows system fonts only, no compiled-in font | STATUS "Director assumptions pending owner" | A font is compiled in and listed in `ATTRIBUTION.md` |
| M7.O5: no NIF source adopted (prep memo `docs/research/nif-sources-2026-10-16.md`) | same table | M7.S4 starts on the sources the owner names |
| CK static strings (M5.A4): no further static analysis of `CreationKit.exe`; the entry retained, not cited | same table; owner row 17 | The entry is cited (allowed) or removed (forbidden) |

## Merge order

The director merges as branches pass review. Lane D's `edit/`, `handlers/` and `xtask/src/parity.rs`
changes go first (item 2), since M5.D1 rewrites the generated half of `gap-matrix.md`;
`defs/` changes go through BX (`cargo xtask codegen`, parity) one family at a time;
`crates/ui-egui/src/gate.rs` changes (lane G, item 3) in their own commits; lanes K and A
(item 4) touch docs only; lanes W and R (item 5) share `crates/ui-egui/src/viewport/` and
the shell, so W2 merges before R2's sync. Shared files (`plan/STATUS.md`,
`plan/execution-plan.md`, `docs/source-registry.md`) in small separate commits.
