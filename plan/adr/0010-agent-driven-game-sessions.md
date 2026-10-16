# ADR 0010: Agent-driven game sessions on the dev profile

- Status: **Approved in principle** (owner ruling 2026-10-15, decisions item 6 and addendum
  item 5; decides `plan/execution-plan.md` M4.O6). Drafted 2026-10-15 as Proposed; the
  ruling's conditions and the answers to the open sub-questions are Amendment 1 below,
  which wins over the text above it where they differ
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

## Amendment 1: owner ruling 2026-10-15 (M4.O6), approved in principle

Recorded from `docs/foothold/owner-decisions-2026-10-15.md` (decision 6) and
`docs/foothold/owner-addendum-2026-10-15.md` (item 5), quoted verbatim:

> **ADR 0010 automation:** APPROVED IN PRINCIPLE. Build an unattended runtime experiment lane
> using isolated game profiles, disposable saves, explicit authorized paths, backups,
> reproducible setup, logs, screenshots, timeout handling, and rollback. Never run tests
> against personal saves or unrelated mod installations. Treat missing observational evidence
> as inconclusive, not passed. Retain manual acceptance where automated evidence is
> insufficient.

> CK fixture creation may initially be owner-assisted. Do not assume unattended CK operations
> work until proven. Automated runtime experiments must retain distinct not-run, pass, fail, and
> inconclusive statuses, with reproducible evidence.

The lane is renamed in the plan: lane T's tasks M4.T1..T10 keep their ids and become
M5 lane C ("runtime"; `plan/execution-plan.md`, M5). The decision and rules 1 to 6 above
stand, with these conditions, which are binding on every session the controller runs:

1. **Isolated profile only.** The controller runs on the dev profile `KitKraft Dev` (MO2
   folder `Kitkraft Dev`) and nothing else; `Default` and any profile not named in the call
   is a refusal (rule 1 already). "Unrelated mod installations" means any other MO2
   instance or game folder: the controller names one MO2 instance and refuses a second.
2. **Disposable saves only.** It loads only the owner's base save named in the call
   (`KitKraftBase` by default, made under M4.O7) and saves only `KitKraftSession-*`;
   personal saves are never loaded, even read-only. A base save that the controller cannot
   prove is the named one (name and SHA-256 recorded at M4.O7) is a refusal.
3. **Explicit authorized paths.** The controller writes only the paths on this list, and a
   test asserts it (M5.C1): `observe\<yyyy-mm-dd>-T<nn>\` (outputs, copied logs,
   screenshots, backups); `docs/game-observations.md` (one appended entry); the dev
   profile's `saves\KitKraftSession-*.sfs` (made by the game on the controller's console
   command); and, for rollback only, the files named in condition 4, restored byte for
   byte from its own backup. The game itself writes its logs and settings as it always
   does; the controller does not write them.
4. **Backups and rollback.** Before launch the controller copies the dev profile's
   `modlist.txt`, `plugins.txt`, `loadorder.txt`, `lockedorder.txt`, its INIs and the base
   save into `observe\<session>\backup\` with their SHA-256. After the game exits it
   compares hashes (rule 2); on a difference it restores the backup copy, records the
   difference as a finding (`plan/STATUS.md`, "Requests") and marks the session
   `inconclusive`. The plugin under test is rolled back with `deploy.rollback` (ADR 0009),
   never by hand.
5. **Reproducible setup.** Each ledger entry names the game build, SFSE build, the
   Console API and Console Command Runner versions, the base save's SHA-256, every
   KitKraft plugin's SHA-256 and the `probes.toml` hash, so the session can be rerun
   (`cargo xtask game-regress`, M4.T7).
6. **Timeouts.** A wall-clock budget per session and a per-probe timeout; a probe that
   times out is `inconclusive (timeout)`, the remaining rows are `not-run (timeout at row
   X)`, and the game is terminated (rule 2).
7. **Statuses.** Every probe row is exactly one of `not-run`, `pass`, `fail`,
   `inconclusive`, with the reason for every status but `pass`. **Missing observational
   evidence is `inconclusive`, never `pass`:** a matcher with no output to match, a log
   that was not written, a screenshot that was not taken where the row asks for one. The
   ledger format (`docs/game-observations.md`) gains `inconclusive` (M5.C1).
8. **Logs and screenshots.** Both are in scope. Screenshots are evidence attached to a
   row, never its verdict (Decision 3 stands); a row whose matcher is `manual` and has only
   a screenshot is `inconclusive` until the owner looks at it.
9. **Manual acceptance retained.** The owner's sessions keep the last word; a row the
   controller cannot close stays the owner's, and `plan/STATUS.md` names it.

Answers to the open sub-questions ("Consequences", last bullet), as the ruling implies
them:

- (a) Accepted in principle, with conditions 1 to 9.
- (b) Screenshots: **in scope** (the ruling lists them). Desktop input injection
  (keystrokes or clicks sent to the game window): **deferred**; the ruling names none and
  the console channel covers every planned probe. A task that finds it needs input
  injection writes the need into `plan/STATUS.md` "Requests" and waits for the owner.
- (c) `crates/game`'s HTTP client and process crate: allowed under MIT or Apache-2.0
  only, listed in this ADR before the first merge (rule 5); the owner confirms the list at
  that merge, as for the workbench's list (ADR 0007, "Dependency list"). A crate under any
  other license waits for the owner.

**Still the owner's:** M4.O7, installing the Console API, Console Command Runner Updated
and (optionally) Console Output To File into `KitKraft Dev` through MO2, setting the INI
switches and creating `KitKraftBase`. Nexus downloads need the owner's login; no agent
downloads or installs them. Lane C's sessions (M4.T1 onward) start only after M4.O7.

## Dependency list (lane C, M4.T3, 2026-10-16)

For the owner's confirmation at the merge (Amendment 1 (c), M5.O2): **`crates/game` adds no
third-party crate.** The console client is HTTP/1.1 over `std::net::TcpStream` (loopback
only), and processes go through `std::process` with Windows' own `tasklist`, `taskkill`
and PowerShell (screen capture). Its dependencies are workspace crates already in
`Cargo.lock`: `serde`, `serde_json`, `toml`, `thiserror`, `regex` (all MIT OR Apache-2.0).
A later HTTP or process crate is listed here first.
