# ADR 0010: Agent-driven game sessions on the dev profile

- Status: **Proposed** (drafted 2026-10-15 at the owner's request; becomes binding when the
  owner records a ruling under `plan/execution-plan.md` M4.O6)
- Date: 2026-10-15
- Scope: policy and design constraints for an agent launching Starfield on the MO2 profile
  `KitKraft Dev`, driving it through a console channel, and recording what it observes.
  Plans lane T (`plan/execution-plan.md`, "Lane T"). No code now.
- Relates to: AGENTS.md "Owner's live game is off limits" (amended by this ADR), ADR 0001
  (clean room: `Starfield.exe` never debug-attached, disassembled or memory-dumped), ADR
  0005 Amendment 3 (evidence grades; the in-game ledger `docs/game-observations.md`), ADR
  0008 (runtime extensions: core never links a framework; third-party plugins are tools),
  ADR 0009 (`deploy --dev` is the only MO2 write path), `docs/research/in-game-agent-testing-2026-10-15.md`
  (the research this rests on)
- Supersedes: the sentence "The agent cannot launch the game; these steps are the owner's"
  (`docs/smoke-rename.md`, the `testplan` handler header) for the dev profile only. The
  `Default` rules are unchanged.

## Context

Every in-game claim KitKraft makes today is closed by the owner: `kit testplan` writes a
console batch and a prose checklist, the owner runs them on `KitKraft Dev`, and types the
result into `docs/game-observations.md`. That is the slowest step in the project and it
scales with the number of recipes, not with the number of agents.

The research note (2026-10-15) found that the community already ships the channel an
agent needs, as SFSE plugins that are installed and run like any other mod:

- **Console API** (stonegdi, Nexus 4280, v2.17.0 of 2026-06-12): `POST
  http://127.0.0.1:55555/console` with a console command as the body returns the
  command's untruncated console output; optional file output. Its SFSE version block
  declares signature-based address independence. Source is public but carries no licence
  file, so it is run, never read into `crates/*`.
- **Console Command Runner Updated** (Bobbyclue, Meridiano, Nexus 7318, v1.11.0 of
  2026-10-14, GPL-3.0 source): runs TOML-listed commands on `DataLoaded`, `GameLoaded`,
  menu, key, equip and quest-stage events; resolves `{Form:plugin:0xID:default}` to the
  runtime FormID.
- **Papyrus logging** (`[Papyrus]` keys in the profile's `StarfieldCustom.ini`) writes
  `Documents\My Games\Starfield\Logs\Script\Papyrus.0.log`.
- `StarfieldConsoleHistory.log` already records every command the game ran (observed).

The LLM game-testing literature (GBQA 2026, TITAN 2025) converged on text and structured
state as the oracle, screenshots as evidence. The closest prior art, `skytest`
(myaiexp/skyrim-headless-mods, 0BSD), is a probe plugin plus `commands.jsonl` /
`trace.jsonl`, isolated profiles and replayable steps.

The owner's live profile `Default` must stay untouched, and the owner may be playing while
agents work (both `Starfield.exe` and `ModOrganizer.exe` were running while this ADR was
drafted).

## Decision (proposed)

1. **Agents may launch and drive Starfield on `KitKraft Dev`**, and only there, through a
   session controller that KitKraft owns (lane T). The owner is no longer the only closer
   of in-game rows; owner sessions remain the arbiter when the controller and the owner
   disagree.
2. **The channel is text.** The controller talks to the game only through: the Console
   API plugin's HTTP endpoint on the loopback interface, files the game or its plugins
   write (console output log, `Papyrus.0.log`, `StarfieldConsoleHistory.log`, saves it
   made itself), and screenshots. It never attaches a debugger, reads game memory, injects
   code, or scans the process (ADR 0001 §2, ADR 0008 §2).
3. **Every expectation is a probe** with a machine-checkable matcher, derived from the
   plugin KitKraft wrote (its forms, positions, names, stages), emitted by `testplan` next
   to the human checklist. A screenshot is evidence attached to a row, never the row's
   verdict; a vision model may annotate ("the scene looks sane"), and that annotation is
   graded D.
4. **Third-party plugins are tools, not sources.** Console API, Console Command Runner
   Updated and Console Output To File are installed by the owner into the dev profile,
   listed in `docs/source-registry.md` as `external-oracle-only (run, never read)`, like
   Spriggit. Their source is not read by a session that edits `crates/*`. KitKraft bundles
   none of them and names the versions it was verified against.
5. **A KitKraft probe plugin is not planned by this ADR.** If lane T finds a row that no
   console command or Papyrus call can answer, it writes a runtime-need record (ADR 0008
   §5) and the plugin is decided there; the `skytest` protocol is the design to follow.

## Rules

### 1. Target confinement (preflight; every one is a refusal, never a warning)

The controller refuses to start when any of these holds, and says which:

- MO2's `ModOrganizer.ini` `selected_profile` is `Default`, is absent, or is not the dev
  profile the call names. (The gate is policy, not confinement, exactly as ADR 0009
  documents for `deploy --dev`.)
- `Starfield.exe` or `CreationKit.exe` is already running (the owner may be playing;
  one game instance at a time), or `ModOrganizer.exe` is running (a second MO2 on the
  same instance is unsafe).
- The dev profile's `StarfieldCustom.ini` lacks the session switches the owner set once
  (M4.O7): `[General] bAlwaysActive=1`, `[Papyrus]` logging on; or the Console API plugin
  is not an enabled mod of the dev profile. The controller **reads** the profile to check
  this; it never writes the switches itself.
- The named base save is missing from the dev profile's `saves\`.
- The plugin under test is not installed through `deploy --dev` (ADR 0009): the
  controller does not install anything.

### 2. Process and session rules

- The game is launched through MO2 with the SFSE executable on the dev profile (the exact
  command line is an M4.T1 fact). The controller waits for the Console API endpoint, then
  for `GameLoaded` (the base save loaded by the console `load` command, M4.T1 grade D
  until observed), then runs the probes in order.
- A session has a wall-clock budget and a liveness check (HTTP answers, console log
  growth). On timeout the controller terminates `Starfield.exe` and records every
  remaining row as `not run (timeout at row X)`.
- The controller ends every session by terminating the game and MO2's helper processes
  it started, and by verifying that the profile's `modlist.txt`, `plugins.txt`,
  `loadorder.txt`, `lockedorder.txt` and both INIs hash the same as before the session.
  A difference is a finding, filed in `plan/STATUS.md` "Requests".
- Saves: the controller loads the owner's named base save and saves only to names it
  owns (`KitKraftSession-<yyyymmdd>-<nn>`); it may delete only those. It never deletes,
  renames or overwrites any other save. (Reading or writing save **files** stays out of
  scope: saving and loading go through the game's own commands.)
- Screenshots go to `observe\<yyyy-mm-dd>-T<nn>\` (gitignored), named per row.

### 3. Writes

The controller writes: `observe\…` (outputs, logs it copied, screenshots), one ledger
entry appended to `docs/game-observations.md`, and the session saves above. Nothing else:
not the game `Data` folder, not `Documents\My Games\Starfield` beyond what the game
itself writes while running, not MO2's `mods\` or `profiles\`. This keeps AGENTS.md's
"nothing writes under…" rule true with one more named exception.

### 4. Evidence

- A probe result is an in-game observation: cite it as `game:<entry-id>/<row>` in the
  ledger's format. A pass upgrades the grade-D claims the row names (`PS-nn`,
  `runtime-slots-by-scale`, the batch-file mechanism) as `docs/game-observations.md`
  already specifies.
- A row that could not run is a result, with the reason, never a blank.
- The controller's own report is never summarised into a pass by hand: `plan/STATUS.md`
  links the ledger entry.

### 5. Layering and dependencies

- Host-side process, HTTP and file work lives in a new L3 crate `crates/game`
  (`kitkraft-game`, next to `crates/io`); it depends on L0 and L1 only. Engine handlers
  `game.session`, `game.probe`, `game.status` (L4) call it, so CLI and MCP share one
  implementation (AGENTS.md "Everything is a command"); the catalog marks them writing
  (MCP `destructiveHint`). ADR 0002 gains the row in the same commit as M4.T3.
- Allowed new dependencies: an HTTP/1.1 client and a process-handling crate under
  MIT/Apache-2.0 only; the list goes into this ADR before the first merge, as ADR 0007
  does for the workbench.
- No game offsets, signatures or framework headers anywhere under `crates/*` (ADR 0008
  §1). The controller knows a port, paths and command strings.

### 6. Other titles

The controller is configured per game (`crates/game/profiles/<game>.toml`: script
extender executable name, console endpoint, log paths, save-load command, profile INI
switches). Starfield is the only filled profile in M4; Skyrim SE and Fallout 4 get stub
profiles naming the known equivalents (ConsoleUtil-class plugins, AutoTest) so the shape
is proven game-agnostic without implementing them.

## Consequences

- AGENTS.md "Owner's live game is off limits" gains the paragraph that permits dev-profile
  sessions under rules 1 to 3 (drafted alongside this ADR; binding with it).
- The sessions are one stage of a longer chain the owner asked for on 2026-10-15: an
  agent handed a brief ("I want a mod that does X") builds, validates, packages, deploys
  to the dev profile, runs the session and reports, with no owner step (M4.T10,
  `kit mod run`, `docs/playbooks/mod-from-brief.md`). This ADR's rules bind that chain's
  game stage; the earlier stages are already governed by ADR 0009 and the recipes.
- `plan/execution-plan.md` gains lane T (M4.T1..T10) and owner rows M4.O6 (this ruling),
  M4.O7 (install the three plugins, set the INI switches, create the base save). Lane T is
  off every M4 critical path and changes no M4 exit criterion; its results can close
  M4.O2 and M4.O4 rows that the owner has not run, with the owner's sessions keeping the
  last word.
- `docs/source-registry.md` gains rows CONSOLE-API, CCR-UPDATED, CONSOLE-OUTPUT-FILE.
- `docs/smoke-rename.md` "In game" and the `testplan` header are reworded when M4.T3
  merges: "run by the owner or by `kit game session` on `KitKraft Dev`".
- Open for the owner in M4.O6: (a) accept, amend or reject this ADR; (b) whether
  screenshots and desktop input injection are in scope for M4.T5 or deferred; (c) whether
  `crates/game` may add its two dependencies before the owner has seen the list.
