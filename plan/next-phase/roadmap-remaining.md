# What is left on the roadmap

Written 2026-11-14 from `main` at `a0b8b642`, read-only. Sources: `ROADMAP.md`,
`plan/STATUS.md`, `plan/execution-plan.md` (M5 to M8), `plan/tickets/README.md`,
`docs/gaps.md`, `docs/foothold/gap-matrix.md`, `docs/ck-parity.md`, and the same files on
branch `sprint/2026-11-09` (worktree `integrator`). Nothing was built or run.

Confidence marks: **[repo]** a file or commit says it; **[inferred]** my reading of several
files; **[unverified]** I could not check it tonight.

## Read this first: `main` is not the whole picture

[done 2026-11-15: the sprint is merged, `23d62954`.] **`sprint/2026-11-09` is 144 commits ahead of `main` and not merged.** [repo: `git log
main..sprint/2026-11-09`, last commit 2026-11-10 18:01, "Merge ws/g @ 8135c8a0"]. It forked
from `main` at `a013c77c` (2026-11-07); `main` has moved 22 commits since (KK-241 to KK-244
and their plan commits). A trial three-way merge reports one conflicting file. I found no
note saying why it is unmerged [unverified: the sprint's completion report is not in the
repo, as with the 2026-11-02 sprint].

What sits only on that branch, from its "Landed 2026-11-10" lines:

- **M7 moved a long way.** `docs/ck-parity.md` "Interface parity" on the branch counts 127
  `observed`, 33 `reproduced`, 4 `improved`, 8 `built, unobserved`, 4 `unresearched`.
  `ROADMAP.md` on `main` still says none is observed. Landed there: the 28 menu flips (C1),
  47 documented interface gaps (C2), M7.K4's label map (105 labels mapped, 93 named gaps),
  six observed edit-dialog layouts plus REFR (M7.W4), the Data dialog's own-master path and
  type-ahead (M7.M2), the Cell View filter (M7.W2), the Warnings window's eleven CK columns
  (M7.M4), the CK feature ranking (M3.H5).
- **Render Window meshes** (M7.R4): references drawn as wireframe or flat-shaded meshes
  from `mesh.geometry`; the mesh index kept in the index cache (M7.R5); V4 and V5 measured
  engine side and in the window.
- **Game-session regression** (M4.T7): every session records a transcript; `cargo xtask
  game-regress` replays recorded sessions against the ledger.
- **Controller work under the send-order ruling**: the routed readiness poll with three fix
  rounds, the hand-written send-order probes, run sheets OW-1 and OW-2.
- **Three items the `main` queue still shows as open**: KK-250, KK-251 (both "queued" in
  `plan/tickets/README.md` on `main`) and the `validate` rule `masters.unflagged`, which is
  tonight's draft KK-262. [repo: the rule is at `crates/engine/src/validate/mod.rs:104` on
  the branch and absent on `main`.]

Consequence: tonight's KK-262 run, and any local-agent run of KK-250 or KK-251, repeats
work that exists on the sprint branch and will conflict with it at merge. This is owner
decision 1 in `README.md`.

Everything below describes `main` and says where the sprint branch differs.

## Per milestone

| Milestone | State | What is left | Blocked on | Ticketable now |
| --------- | ----- | ------------ | ---------- | -------------- |
| M0, M1, M2 | done [repo] | nothing | - | - |
| M3 | closing | The `ck` write style; CK fixtures F00 to F14 beyond F29/F30; recompression question F07; inline-text encoding F13; overhaul save/reload row O5-15 | CK sittings (owner OK per sitting); O5-15 needs a dev-deployed overhaul (the controller refuses the hand-installed copy) | O5-15 as an agent game session once the overhaul is deployed with `deploy --dev`. The `ck` write style is not ticketable until fixtures exist |
| M4 | closing | W12 rename-package install; session 2 rows O4-01 to O4-07 (partly run by agent sessions on 2026-11-03: actors, ownership, scripts, mini-site; see the ledger); CK fixtures F20 to F28 | Owner sitting for F20, F23, F27, F28 (the CK's save box is not reachable by UI Automation); the sitting plan `plan/owner-sitting-2026-11-06.md` has seven open questions | The remaining recipe sessions (location, terminal, colourway, workshop, fabricator) as agent sessions; the desk half of the fixture sitting is on the sprint branch (C4) |
| M5 | partial | CK differential for every type but CELL and REFR (183 of 185 `not-run`); save/reload for every type but CELL and REFR; the confirming send-order session (owed since 2026-11-07; OW-1 "not run past the base save" on the sprint branch); FTH-EXP game rows | Game time on `KitKraft Dev` (agent sessions are allowed; the director does not start the game unasked); CK sittings | Probe and testplan work that makes sessions cheaper (see `mod-ideas-support.md`, holes H3 and H6) |
| M6 Cockpit | built; not signed off | The end-to-end run from the real window (M6.G4's V1), the real-window checks list, M6.O4 sign-off; window rows of the performance table; lane X (keyboard, scaling, screen reader, first run) is DEFER | Owner at the desk while an agent drives `kit-ui.exe` (decision 2026-11-05 item 8); run sheet OW-2 is written on the sprint branch | KK-246 to KK-249 (queued), KK-263 (tonight). Lane X could be cut into tickets if the owner lifts DEFER |
| M7 CK parity | in progress (well ahead on the sprint branch) | Sitting-dependent rows: hotkeys and camera bindings, context menus, the Object Window category tree (M7.S2, M7.W1), remaining Preferences pages (M7.M3), WEAP dialog, LCTN tab names, Edit > Undo; shortcuts (M7.M5); the edit dialog beyond scalar fields (struct, list, lstring, FormID fields: `docs/gaps.md` first row); New/Duplicate and placement from the windows in a real window (M7.E3, E4); the M7 exit plugin `KC_F31.esp` through CK re-export; A/B sign-off M7.O6 | The owner-assisted sitting (items 11 and 12 of 2026-11-05); the Data-dialog question (below); a Claude-agent design split for the edit dialog (it touches the write path, `dialogs/editor.rs` is 1,900 lines) | After the sprint merge: member-editing tickets for one field kind at a time (lstring, FormID picker, list), each headless; M7.K5-style parity tests. Before the merge: nothing safe in `crates/ui-egui` (the branch changed 70 files) |
| M8 Beyond | sketch only | Everything; no exit criteria; each write needs an ADR 0007 amendment and the agent panel needs a journal ADR | Owner: which features, and the ADRs | Engine-side, UI-free pieces are ticketable now: see `beyond.md` "Ticketable before M7 closes" |
| Later | unscheduled | NIF and materials beyond the viewport, procedural placement, runtime extensions, other titles, release | Owner decisions 5 and 6 (SS2 contact, publishing) | Hosted CI is the one piece with no dependency (`docs/gaps.md` last row; owner's call) |

## Schema and data depth (cross-cutting)

- 186 record types, 186 definitions (the gap-matrix headline on `main`); typed-readable 144
  pass by MX's measure, which an all-opaque inventory definition satisfies. [repo]
- **Schema depth by ticket is nearly exhausted.** After tonight's seven types, the depth
  generator finds no other candidate: SCEN, GBFM, REFR and NAVM are its hard cases, and the
  rest keep their unknowns inside component blocks. [repo: `BATCH-20261113.md` "Could not
  ticket"]
- The two remaining schema levers are format work, not transcription tickets: the
  **component-name decider** for `BFCB` bodies (55 of 64 component kinds, 349,154 blocks;
  [2026-11-15: KK-280 to KK-288 typed eight more kinds, 22 of 64 typed; ADR 0015 accepted
  and the decider being built, KK-292])
  and the **reflection reader** (`REFL`, `XNSE`, `PTCL`, `PCCC`, `RDIF`, `PSDF`; 46,863
  payloads, self-describing). Both need a design split by a Claude agent before they are
  1.5-hour tickets. [repo: `docs/gaps.md`, STATUS "Next" item 5]
- Typed-writable is proven for 24 types of 186; tonight's KK-258 to KK-260 and KK-265 add up
  to twelve. All of these share `authoring_tests.rs` and `evidence.toml`, so they chain one
  per night unless queued as a list.
- CK differential 2 types, game 5, save/reload 2. These move only with game and CK time.

## Blocked on the owner, in one list

Rulings:

1. Merge `sprint/2026-11-09` (or say what it waits for). [done 2026-11-15, `23d62954`]
2. Whether the Data dialog follows the 2026-11-07 ruling or the CK for a master-flagged
   active file. The sprint branch implements "path field follows the ruling, list keeps the
   CK's rule" as a default in force (RULINGS P1); it needs a yes or no.
3. Whether the Cell View's Base column shows the EditorID.
4. Lane X: stays DEFER, or opens.
5. M8: which features first, and the two ADRs (write scope amendment, MCP journal).
6. Converting an existing file's header to a small, medium or full master: proposed by
   M3.H5 as an "M4 candidate" (the only P1 CK feature with no command). Needs F06, F11, F26
   evidence or an explicit "build it at grade D".
7. Hosted CI on the public repository.
8. The seven questions at the end of `plan/owner-sitting-2026-11-06.md`.

At the desk (game window, real CK, GPU):

- The M6 end-to-end sitting (OW-2) and M6.O4.
- The CK fixture and interface sitting (F20, F23, F27, F28, F31; hotkeys, context menus,
  category tree, Preferences, WEAP dialog, Undo).
- The send-order confirming session (OW-1), or an OK for an agent to run it.
- Real-window checks: load from a real MO2 profile, Object Window speed, viewport V3 and
  the meshes' time-to-drawn and GPU memory.
- Housekeeping from STATUS rows 5, 8, 11: the folder move is done in practice (the repo is
  at `C:\Modding\kitkraft`) [inferred]; the install and walkthrough rows still read pending.

## Ticketable now, without the owner

In rough value order. Items marked (S) wait for the sprint merge because they touch files
the branch changed.

1. Holes from the mod analysis: `mod-ideas-support.md`, first batch (ten items; drafts for
   seven of them are in `drafts/`).
2. Beyond-CK engine pieces with no UI: plugin delta and clean check, text export, search
   index, rule suppressions (`beyond.md`).
3. Typed-write evidence for the actor and weapon-adjacent types tonight's batch does not
   cover: `NPC_`, `LVLN`, `AMMO`, `PROJ`, `ZOOM`, `PACK`, `RSPJ`, `PERK`.
4. Clone-sample corpus tests beyond the eight types covered after tonight.
5. The design notes that unblock larger work (Claude agent, docs only): component-name
   decider [done 2026-11-15: `docs/design/component-blocks.md`, KK-281]; reflection reader; the edit dialog's member editing (S); the
   `env!("CARGO_MANIFEST_DIR")` convention.
6. `docs/gaps.md` and `ROADMAP.md` reconciliation after the sprint merge (S): "Where we are"
   is dated 2026-10-30 and predates two sprints.
