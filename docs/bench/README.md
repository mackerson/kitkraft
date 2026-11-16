# Bench sheet, 2026-11-16 (from about 21:30)

The one document the owner and the driving agent follow, top to bottom, at tomorrow's
at-desk bench. It orders the lane documents and links them for detail; it repeats only
the commands. Nothing in it has run against the game, the CK or the input executor yet;
"Tonight's dry runs" is what did run (2026-11-15, no game, no CK, MO2 left as the owner had
it).

| Lane document | What it holds |
| ------------- | ------------- |
| [`docs/game-sessions.md`](../game-sessions.md), run sheets OW-1 and OW-2 | the run sheets, their predictions and how to read results |
| [`ow1-prep.md`](ow1-prep.md) | OW-1's preflight script and its decision tree (section 4) |
| [`verify-mod.md`](verify-mod.md) | the verification plugin, its 106 probe rows, entering the results |
| [`ck-sitting.md`](ck-sitting.md) | the CK batch, the first-minute checks, the CK after-step |
| `C:\Modding\kitkraft-observe\scripts\input\README.md` (outside the repo) | the input executor, the 15-minute experiment, scenarios SC-1 to SC-5 |
| `plan/adr/0010-agent-driven-game-sessions.md` Amendment 2, `plan/adr/0012-ck-interface-clean-room.md` | the rulings: input and focus allowed, a launch needs the owner's go, a CK save needs the owner's OK per sitting |

Conventions: `$wt` is the worktree the bench runs from (`C:\Modding\kitkraft-worktrees\bench-ck`
unless the director names another); `$kit = 'C:\Modding\kitkraft\.targets\x\release\kit.exe'`;
`$env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'`;
`$base = '--base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf'`
(written out in full below). Profile `Kitkraft Dev` only, never `Default`.

## Release binaries

| Binary | SHA-256 | Built |
| ------ | ------- | ----- |
| `.targets\x\release\kit.exe` (package `kit-cli`) | `0bbcfc6d3b7b5341784cee922af8950cfdcee851e55d10eb9488315fa2317172` | 2026-11-15 23:06 from `a618e7d6` (this branch after `git merge main` at `88aeab48`; later commits on the branch touch only docs and `docs/bench/ck-batch/build.ps1`) |
| `.targets\x\release\kit-ui.exe` (package `kit`) | `c87a5c6d42a6d4ea470f27bbb587f1f2b11196bfead2370ed1cfbd87e1567453` | the same build |

If `main` moved overnight (KK-360 to KK-362 may land) and the bench should run it, rebuild
**before** the window, never while the game runs (about 2.5 minutes warm), from `$wt` at the
commit to run, and record the new hashes in the first ledger reading:

```powershell
$env:CARGO_TARGET_DIR = 'C:\Modding\kitkraft\.targets\x'; $env:CARGO_BUILD_JOBS = '4'; cargo build --release -p kit-cli -p kit; Get-FileHash -Algorithm SHA256 C:\Modding\kitkraft\.targets\x\release\kit.exe, C:\Modding\kitkraft\.targets\x\release\kit-ui.exe
```

## Before the bench (owner, 5 minutes)

1. Close Starfield, the CK and anything of SFSE.
2. Open MO2, select the profile **`Kitkraft Dev`**, then **close MO2**. Tonight's
   preflights failed only on this (MO2 open, selected profile `Default`).
3. Nothing else needs renewing: the base save `KitKraftBase.sfs` hashes as the run sheets
   say, the rename plugin is deployed and enabled, the Console API is on port 45555
   (checked tonight, below).
4. Load order: `KitKraftFirstPOI.esm` (enabled in `Kitkraft Dev`) must load **before**
   `KitKraftVerify.esm`. `kit deploy enable` appends the verification plugin last, so this
   holds unless someone reorders by hand; the mini-site, scripts and hostile-fixture mods
   stay disabled ([`verify-mod.md`](verify-mod.md), "Load order").
5. Leave the primary monitor free: the CK and the game must open there (screenshots
   capture only the primary screen).

## Owner answers (one pass, before anything runs)

Answer every row at once; the agent records the answers verbatim in the first ledger
reading and passes the save words to `-OwnerSaveOk`.

| # | Question | Pick | Recommended |
| - | -------- | ---- | ----------- |
| A1 | Game launch go for this evening: OW-1 (one launch, a second only on a retry), the verification mod (one), OW-2 (two in part 1, one in part 2)? | yes / no | **yes** |
| A2 | Run the input experiment in game, the agent taking the focus, desk free about 15 minutes, plus about 10 for scenarios? (Amendment 2 says nothing is launched until you pick a design; this is that trial of the external executor) | yes / no | **yes** |
| A3 | Include the gamepad step (`--pad`: Windows' built-in `InputInjector` makes a virtual pad; no driver install)? | yes / no | **yes** |
| A4 | May scenario SC-5 save through the pause menu (a game save not named `KitKraftSession-*`, which the session rules otherwise forbid)? | yes / no | **no** (skip SC-5) |
| A5 | CK bench batch: may the agent save each `KCB_*.esp` in place in the CK (File > Save Plugin, into its own dev mod)? | yes / no | **yes** (without it `ck_reexport` stays `not-run`) |
| A6 | F28 by the agent: duplicate the coffee maker and save `KC_F28.esp` into `overwrite\`? | yes / no | **yes** |
| A7 | Fixture 1, F23: reference `Starfield.esm:1B21E0`, Position X = `1234.5`? | yes / other | **yes** |
| A8 | Fixture 2, F20: `Crate_MilitarySupply_01` as the STAT and `KC_F20_Cell` as the EditorID? | yes / other | **yes, both** |
| A9 | Fixture 3, F27: Freestar Collective Embassy and `PlayerFaction`? | yes / other | **yes** |
| A10 | Fixture 4: Part B before or after the four fixture saves? | before / after | **after, only if time remains** |
| A11 | Fixture 5: B10 (the 12 submenus)? | yes / last-if-time / no | **last, only if time remains** |
| A12 | Fixture 6: also copy each saved fixture into `C:\Modding\kitkraft\observe\`? | yes / no | **no** (one copy in `C:\Modding\kitkraft-observe\`) |
| A13 | Fixture 7: who deletes the CK's files in MO2's `overwrite\` after each launch? | owner / agent | **owner** (agents write under MO2 only through `kit deploy`) |

Asked later, not now: the M6.O4 sign-off (end of block 4). The reasons behind A5 to A13 are
in [`ck-sitting.md`](ck-sitting.md), "Owner answers needed".

## The run

Order changed from the draft (preflights, OW-1, verification mod, input, CK, OW-2), for
two reasons:

- **The CK batch goes last.** Its `deploy enable` puts 13 `KCB_*` plugins into
  `Kitkraft Dev`, and every later game session would load them: `KCB_F23k.esp` moves a
  reference in the apartment cell the sessions use, `KCB_G8_Planet.esp` clones planet and
  star records (which [`verify-mod.md`](verify-mod.md) keeps out of the game because they
  can change world generation and cost a session), and every runtime slot after them
  shifts. Last, nothing loads them in game, the owner is free while the groups run, and
  OW-2's real `KC_F31.esp` exists by then.
- **The input experiment runs inside OW-2 part 1's session.** It needs the game up and
  idle after a session's probes. Only a session started with `--start` stays open (the
  one-shot `kit` CLI ends the game when it returns: [`game-sessions.md`](../game-sessions.md),
  "Commands"), and the window's **Launch...** is that start. OW-2 part 1 launches one
  anyway, so the experiment costs no launch of its own.

| Block | What | Who | Minutes | Running total |
| ----- | ---- | --- | ------: | ------------: |
| - | Before the bench, owner answers | Owner | 10 | 0:10 |
| 0 | Preflights (no game) | Agent | 10 | 0:20 |
| 1 | OW-1, send order | Agent, owner away from the screen | 12 (+6 a retry) | 0:32 |
| 2 | Verification mod session | Agent | 25 | 0:57 |
| 3 | OW-2 part 1, with the input experiment and scenarios in its session | Agent drives `kit-ui`, owner watches; desk free during input | 45 | 1:42 |
| 4 | OW-2 part 2 (`KC_F31.esp`) and part 3, M6.O4 sign-off | Agent drives `kit-ui`, owner watches | 45 | 2:27 |
| 5 | CK batch: deploy, G1 alone (owner watches), groups G2 to G8 | Agent; owner free after G1 | 50 | 3:17 |
| 6 | Close-out: evidence check, `cargo xtask parity`, gate, commit | Agent (no game) | 30 | 3:47 |
| 5+ | Optional CK, in this order: fixture twins (25), F28 by the agent (8), F23, F20, F27 with the owner's hands (15 each), failed groups split by type (5 a type) | Agent, owner for F2x | up to 78 | up to 5:05 |

Starting at 21:30 the core ends about 01:20; with every optional CK step about 02:35.

**If only 60 minutes:** before and answers, block 0 (skip the input and CK self-tests),
block 1, block 2: about 57 minutes. The verification session is the most evidence per
minute (up to 47 game-column types and 21 save/reload in 25 minutes); OW-1 must come first.
Evidence entry and the gate run afterwards without the owner.

**If only 2 hours:** the 60-minute cut, then block 5 core (deploy, G1 watched, the seven
groups run while the owner is away from the desk): about 1:47, then block 6. Up to 56 new
`ck-differential-tested` types for 50 minutes beats OW-2 and the input trial, which change
one milestone row and a design choice and need the owner at the desk for 90 minutes; they
move to the next window together.

### Block 0: preflights (no game, MO2 closed)

```powershell
cd $wt
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\bench\ow1-preflight.ps1 -Kit $kit
# the verification plugin (pure builds, no MO2 write):
& $kit papyrus compile docs\bench\verify-mod\scripts\KitKraft\VerifyReport.psc out\verify\Scripts --sources-zip "C:\Program Files (x86)\Steam\steamapps\common\Starfield\Tools\ContentResources.zip"
& $kit build docs\bench\verify-mod\verify-mod.toml out\verify\KitKraftVerify.esm --overwrite
& $kit validate --plugin out\verify\KitKraftVerify.esm --plugins Starfield.esm
& $kit roundtrip out\verify\KitKraftVerify.esm --verify
& $kit package out\verify\KitKraftVerify.esm out\verify\pkg --scripts out\verify\Scripts\KitKraft\VerifyReport.pex --spec docs\bench\verify-mod\verify-mod.toml --plugins Starfield.esm --overwrite
# OW-2's inputs:
& $kit build docs\smoke-rename.toml out\OldSovietMarksmanRifle.esm --overwrite
& $kit testplan out\OldSovietMarksmanRifle.esm out\rename-plan --plugins Starfield.esm
# input and CK desk tests (launch nothing):
cd C:\Modding\kitkraft-observe\scripts\input; python selftest.py --no-live; python experiment.py --dry-run
cd C:\Modding\kitkraft-observe\scripts; powershell -File .\ck-plugin.selftest.ps1; powershell -File .\ck-sitting.selftest.ps1
```

- **Pass:** preflight `fails: 0` and `would start session ...`; plugin SHA-256
  `a9632f11...e39faa`; validate 0 errors (1 warning `cell.actor-without-navmesh` is known);
  roundtrip identical; rename plugin `9c0b28a6...78ea27`, rename `probes.toml`
  `d84e011a...a55dc05`; input 0 failed and 10 PASS; CK self-tests `0 failed`.
- **Failure:** [`ow1-prep.md`](ow1-prep.md) section 4 for every preflight rule. A plugin
  hash that differs means `kit` changed: stop and tell the director before any session.
- **After:** paste the preflight's list-file hashes into the notes (OW-1's "before").

### Block 1: OW-1 (must precede every other session)

Run sheet: [`game-sessions.md`](../game-sessions.md), "Run sheet OW-1"; commands with
durations: [`ow1-prep.md`](ow1-prep.md) section 5.

```powershell
cd $wt
& $kit deploy enable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev" --dry-run   # expect: already enabled, nothing to change
& $kit game session C:\Modding\MO2-Starfield crates\game\probes\send-order.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger $wt\docs\game-observations.md --dry-run
# no refusal: the same without --dry-run, synchronously; hands off for about 5 minutes
Get-Process Starfield, sfse_loader, ModOrganizer -ErrorAction SilentlyContinue   # nothing
```

- **Pass:** SO-01 `pass`; at least one late line; every reply in send order ("Held").
- **Failure:** [`ow1-prep.md`](ow1-prep.md) section 4 (a timeout now carries a
  `diagnostics at ...` finding and `screenshots\stuck-<n>.png`). One retry in the window. A
  contradiction (SO-05 `pass` with SO-06 `fail`, or a later line before an earlier one):
  **stop the evening's sessions** and send it to the director at once (R17).
- **After (same night, during block 2's session):** the writer's reading under the ledger
  entry ([`game-sessions.md`](../game-sessions.md), "Reading the result"). Keep the rename
  plugin enabled: OW-2 needs it (disable it at the end of block 4).

### Block 2: the verification mod

Detail: [`verify-mod.md`](verify-mod.md), "Commands for tomorrow".

```powershell
cd $wt
& $kit deploy out\verify\pkg\KitKraftVerify C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev" --dry-run   # then without
& $kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftVerify --dev --profile "Kitkraft Dev" --dry-run   # then without
& $kit testplan out\verify\KitKraftVerify.esm out\verify\plan-dev --profile "C:\Modding\MO2-Starfield\profiles\Kitkraft Dev"
python docs\bench\verify-mod\make_probes.py out\verify\plan-dev\probes.toml out\verify\probes.toml   # note the slot it prints
& $kit game session C:\Modding\MO2-Starfield out\verify\probes.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger $wt\docs\game-observations.md --dry-run
# no refusal: the same without --dry-run; hands off, 8 to 12 minutes of rows
& $kit deploy disable C:\Modding\MO2-Starfield --mods KitKraftVerify --dev --profile "Kitkraft Dev" --dry-run   # then without
```

- **Pass:** the slot `make_probes.py` prints is the verification plugin's place after
  `KitKraftFirstPOI.esm` in the order `deploy enable` wrote (tonight `0A`, with the rename
  plugin enabled); rows pass as the type table in [`verify-mod.md`](verify-mod.md) predicts.
  V-P29 (PKIN) may fail: it answers an open question.
- **Failure:** a launch or base-load timeout: [`ow1-prep.md`](ow1-prep.md) section 4. A
  failed row fails only itself; read its `seen`. V-P10 failing alone points at the load
  order (above).
- **After (same night):** `python docs\bench\verify-mod\propose_evidence.py C:\Modding\kitkraft-observe\<session>\report.json`,
  read each row's `seen`, enter the entries in `docs/schema/evidence.toml` as
  [`verify-mod.md`](verify-mod.md) "Entering the results" says, then
  `cargo test -p kitkraft-engine --lib every_evidence_pointer_resolves` and `cargo xtask parity`.

### Block 3: OW-2 part 1, with the input experiment in its open session

Run sheet: [`game-sessions.md`](../game-sessions.md), "Run sheet OW-2", Part 1, steps 1.1
to 1.11. The agent drives `kit-ui` by computer use (that window only); the input executor
is a separate script, not computer use.

1. Gate step: the rename plugin is already enabled (block 1). Steps 1.1 to 1.7:
   `& $kit ui --dev` from `$wt`; Cockpit, Review, Status, Preview deploy, Deploy, Preview
   session with `out\rename-plan\probes.toml`, **Launch...**, wait for phase idle. Note
   the elapsed seconds the panel shows: the session's budget is 1,800 s from launch.
2. **Input experiment** (A2 yes), at once after 1.7, game in front, borderless or windowed:

   ```powershell
   cd C:\Modding\kitkraft-observe\scripts\input
   python kkconsole.py getpos          # three numbers: the console answers
   python experiment.py --ask --pad    # drop --pad if A3 is no; hands off about 3 minutes; answer the A5 question in the terminal
   ```

   - **Pass:** A1 (W moves the player), A2 (mouse turns), A5 (E opens the crate's menu);
     the report is `runs\<stamp>\report.json`, every input in `inputs.jsonl`.
   - **Failure:** the README's "What the results mean" table. A1 failing even with `tcl`:
     check that the game does not run elevated, then stop the input part.
3. **Scenarios**, only if A1, A2 and A5 passed and the panel's elapsed time is under
   1,400 s (each takes a minute or two):
   `python run_scenarios.py --only SC-3 --ask`, then `SC-1`, `SC-4`, then
   `python run_scenarios.py --only SC-2 --ask --param npc_ref=0000598D`. SC-5 only with A4
   yes (`--allow-menu-save`).
   - **SC-2's NPC:** `NoelREF`, `Starfield.esm:00598D` (runtime `0000598D`; base `NPC_`
     `Constellation_Noel`), a persistent `ACHR` placed in `CityNewAtlantisLodgeInt`
     (`Starfield.esm:06BB29`), the Lodge cell the sessions `game-20261101-04` and
     `game-20261103-01` used (`kit cell --cell CityNewAtlantisLodgeInt --plugins Starfield.esm`).
     `player.moveto` finds her wherever the base save's quest state put her. Fallback:
     `VascoREF`, runtime `000057BE`, same cell. If neither answers to `prid <id>`, pick at the
     bench: `coc CityNewAtlantisLodgeInt`, open the console, click an NPC (the console
     shows its ref ID), close the console, and pass that ID.
4. Steps 1.8 to 1.11 (screenshot re-run, Record in Ledger, Stop, relaunch).

- **Pass (OW-2 part 1):** a ledger entry started from the window, six statuses filled
  (`ck_open`, `ck_reexport` `not-run`), the screenshot shown, the manual row recorded.
- **Failure:** a stuck key: `python kkinput.py release`, or tap the key; `tcl` may be left
  on after an A1 diagnostic (type `tcl` once). If the budget runs out mid-input, the game
  ends and the executor's guard stops at its next 10 ms check; go to 1.11 for a fresh
  session.
- **After (same night):** the writer's reading of the window's ledger entry names the
  input run folder (`runs\<stamp>\`, `inputs.jsonl`): ADR 0010 Amendment 2 wants input
  recorded with the session, and the controller's `report.json` does not hold it.

### Block 4: OW-2 part 2 and part 3, the M6.O4 sign-off

Run sheet: [`game-sessions.md`](../game-sessions.md), "Run sheet OW-2", Part 2 (2.1 to
2.13) and Part 3.

- Part 2 makes `out\f31\KC_F31.esp` in the window (set, clone, place), then validate,
  roundtrip, testplan, deploy, `deploy enable --mods KC_F31` (dry run first), one session
  from the window (2.11, 2.12), and `deploy disable --mods KC_F31` (2.13).
- Part 3: one note per check in the writer's reading.
- Then offer the owner M6.O4's sign-off (revised T3, `docs/design/workbench-performance.md`);
  his words verbatim into the reading.
- Last: `& $kit deploy disable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev" --dry-run`, then without (the list files hash as before block 1, but for the known `plugins.txt` line).
- **Pass:** `KC_F31.esp` SHA-256 recorded; `parse`, `byte_roundtrip`, `game_runtime` filled;
  `save_reload` by 2.12.
- **Failure:** OK disabled in 2.2: record why and stop part 2 (`blocked`). Session faults:
  [`ow1-prep.md`](ow1-prep.md) section 4.
- **After:** the two readings; `KC_F31.esp`'s hash into the reading for the CK lane.

### Block 5: the CK batch (game closed for the rest of the evening)

Run sheet: [`ck-sitting.md`](ck-sitting.md), "Run sheet", steps 2 to 12.

```powershell
$m = 'C:\Modding\kitkraft-observe\ck-2026-11-16\plugins\manifest.json'
# 1. each `kit deploy ... --dev --dry-run` line of C:\Modding\kitkraft-observe\ck-2026-11-16\plugins\deploy.txt, then without;
#    then its `kit deploy enable` line (one --mods per mod), --dry-run, then without
cd C:\Modding\kitkraft-observe\scripts
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 11 -DryRun     # every plugin "deployed and enabled"
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 11 -Only KCB_G1_Core.esp -Save -OwnerSaveOk "<A5 words>, 2026-11-16"   # owner watches
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 12 -Only KCB_G2_Items.esp,KCB_G3_Weapons.esp,KCB_G4_Objects.esp,KCB_G5_World.esp,KCB_G6_Actors.esp,KCB_G7_Quest.esp,KCB_G8_Planet.esp -Save -OwnerSaveOk "<same words>"
# A5 no: the same lines without -Save and -OwnerSaveOk (ck_open only)
```

- **Pass:** G1's seven first-minute checks ([`ck-sitting.md`](ck-sitting.md), "The first
  minute at the bench"): CK on the primary monitor, controls found, `KCB_G1_Core.esp` the
  active file, title and version 1.16.244.0, load 80 to 180 s, in-place save with no
  dialog, clean exit and postflight `ok`.
- **Failure:** any of checks 2 to 6 differs: stop file, read the screenshots together,
  decide before the next plugin. A group that crashes or hides its cause: its types one by
  one (`-Kinds type -Only KCB_T_A.esp,...`, block 5+).
- **After (same night, while the next plugin runs):** per saved plugin,
  `powershell -File $wt\docs\bench\ck-batch\ck-ingest.ps1 -Sitting <sitting folder> -Manifest $m -Kit $kit`,
  read `ingest\verdicts.json`, then the same with `-Apply`, then
  `cargo test -p kitkraft-engine every_evidence_pointer_resolves`.
- **End of the CK block:** the `kit deploy disable` line of `deploy.txt` (`--dry-run`
  first); the owner switches MO2 back to `Default` before playing.

### Block 6: close-out (agent, no game)

```powershell
cd $wt
$env:CARGO_TARGET_DIR = 'C:\Modding\kitkraft\.targets\opus-docs'
cargo test -p kitkraft-engine --lib every_evidence_pointer_resolves
cargo xtask parity
cargo xtask ci
git add docs\game-observations.md docs\ck-observations.md docs\schema\evidence.toml docs\record-parity.md docs\foothold\gap-matrix.md
git commit   # never --no-verify; the gate passed on this commit
```

The hand-off lists each block's result in AGENTS.md's reporting block.

## Stop and abort, in one place

| What is running | Clean stop | Hard stop |
| --------------- | ---------- | --------- |
| Input executor (block 3) | **Pause** key or **Ctrl+Alt+End** (work while the game has focus); or `New-Item C:\Modding\kitkraft-observe\scripts\input\ABORT` (nothing starts while it exists; delete it to run again); or Alt+Tab away (the guard stops at its next 10 ms check). Every key and button is released within about 20 ms | Ctrl+C in its terminal; a key that looks stuck: `python kkinput.py release` with the game in front |
| Session from the window (blocks 3, 4) | **Preview stop**, **Stop...** and the confirm (`game.stop`: game ended, logs copied, profile verified, ledger entry) | close `kit-ui`: a session belongs to the engine that started it, and that engine going away stops it the same way |
| Session from the CLI (blocks 1, 2) | none: `kit game stop <id>` from another terminal cannot reach it (a one-shot CLI's session belongs to that process). Let its bounds end it: 300 s launch, 180 s load, 120 s a row, 1,800 s session | Ctrl+C, then the owner closes the game and MO2. The controller's profile check and rollback did not run: compare the four list files with the preflight's hashes; their backups are in `C:\Modding\kitkraft-observe\<id>\backup\`; restoring them is the owner's call; report it |
| CK batch (block 5) | `New-Item C:\Modding\kitkraft-observe\ck-2026-11-16\STOP` (the running plugin finishes its clean-up; remove the file before the next batch) | Ctrl+C in the batch window, then `powershell -File .\ck-cleanup.ps1 -SittingDir C:\Modding\kitkraft-observe\ck-2026-11-16\<sitting folder>` (add `-SaveAllowed <plugin>.esp` for a saving run) |
| The owner needs the desktop now | switch windows: the input guard and the CK scripts both stop when their window is not in front; the CK frame closes the CK and restores its settings | as above |

After any abort, before anything else: `Get-Process Starfield, sfse_loader, CreationKit, ModOrganizer`
prints nothing, and (CK) the postflight line in `session.md` is `ok`.

## Known unknowns

- **Never in a real session:** this `kit` build; the R41 pacing poll after a `load`; the
  timeout diagnostics (`stuck-<n>.png`); the launch wait's send-order count can still time
  out if the game drops a launch poll's answer ([`ow1-prep.md`](ow1-prep.md) section 1).
- **Console commands never sent by a session:** `getglobalvalue`, `player.getinfaction`,
  `player.hasperk`, `player.getav`; grade D (written, never observed): `player.additem` +
  `getitemcount`, `player.equipitem` + `getequipped`, `getstage`; for input:
  `getangle`, `setangle`, `placeatme`; `isindialoguewithplayer` (a Fallout 4 function, may
  not exist). `help` answers often arrive one request late.
- **Verification mod:** the reporter's `OnLoad` on a placed activator after leave and
  return; PKIN's `getisid` (answered `0.00` before); 21 types are presence only.
- **The CK runner has never driven the real CK:** posted `BM_CLICK` on "Set as Active File";
  whether `EditorWarnings.txt` is written during the load; what the CK does with
  EditorID-only `[[new]]` records of types with required members; G8's planet records.
- **Input:** whether Starfield takes `SendInput` scan codes and relative mouse (UIPI blocks
  them if the game runs elevated); the pad through `InputInjector` (Steam Input may sit
  between); whether the console answers while a menu is open; capture of the DX12 window
  (black in exclusive fullscreen); SC-1 to SC-5 are untested and assume menu layouts.
- **The window:** OW-2's real-window checks (KK-032 to KK-036, KK-040) have run only
  against fakes; the input part shares part 1's 1,800 s budget.
- **MO2:** rewrites `ModOrganizer.ini` and `plugins.txt` at launch (known, no grade change);
  the selected profile may stay `Kitkraft Dev` afterwards.

## Tonight's dry runs (2026-11-15, 23:08 to 23:12, with the release `kit.exe` above)

MO2 was open with `Default` selected (the owner is playing), so every refusal below is
that, or a plugin not deployed by the lane rules; nothing was written under MO2 or the
game folder.

| Dry run | Result |
| ------- | ------ |
| `ow1-preflight.ps1` | `fails: 4`: `processes` (ModOrganizer running), `selected profile` (`Default`), dry run `[selected-profile]` and `[running]`. PASS: base save `fbed2cc2...eb5ccf`, plugin `9c0b28a6...78ea27`, port 45555 free and not excluded, disk 91.9 GB. List files: `modlist.txt` `bfa10ad4...`, `plugins.txt` `286a54cf...`, `loadorder.txt` `46b28b56...`, `lockedorder.txt` `9962fbb0...`; session id `2026-11-16-T01` |
| OW-1 `deploy enable --mods OldSovietMarksmanRifle --dry-run` | refused `[running]` only; all three lines "already enabled / already last" |
| Verification plugin build chain | `VerifyReport.pex` compiled; `KitKraftVerify.esm` SHA-256 `a9632f11...e39faa` (as prepared); roundtrip identical (81,840 bytes, 61 records); package written |
| Verification `deploy --dev --dry-run` | refused `[live-profile]` only; plan: 2 files into `KitKraft Dev - KitKraftVerify` |
| Verification `deploy enable --dry-run` | refused `[running]` and `[mod-missing]` (not deployed tonight, by the lane rules) |
| Verification `testplan` + `make_probes.py` | 106 rows, slot `0A` |
| Verification `game session --dry-run` | refused `[selected-profile]`, `[running]`, `[plugin-not-deployed]` |
| OW-2 inputs | rename plugin `9c0b28a6...78ea27`; `out\rename-plan\probes.toml` `d84e011a...a55dc05` (as on 2026-11-02); `game session --dry-run` refused `[selected-profile]`, `[running]` only |
| CK `kit deploy --dev --dry-run`, 13 group and twin plugins | each refused `[live-profile]` only |
| CK `kit deploy enable --dry-run`, 13 mods | refused `[running]` once and `[mod-missing]` 13 times. **Fixed tonight:** `deploy.txt`'s enable and disable lines named all mods after one `--mods`, which the CLI refuses (`unexpected argument`); it takes one value per `--mods`. `deploy.txt` was corrected in place (original kept as `deploy.txt.bak-20261115`) and `build.ps1` writes it so |
| `ck-batch.ps1 -DryRun` | 13 rows listed; each "not deployed" and "not enabled", with the fixing command |
| `ck-plugin.selftest.ps1`, `ck-sitting.selftest.ps1` | 0 failed each |
| Input `selftest.py --no-live`; `experiment.py --dry-run`; `run_scenarios.py --dry-run --param npc_ref=0000598D` | 0 failed; 10 PASS, 0 FAIL (fake console); SC-3 PASS, SC-1/2/4 inconclusive by design (screenshot-decided), SC-5 skipped (no `--allow-menu-save`) |
