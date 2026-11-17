# Bench sheet, 2026-11-16 evening (from about 21:30, possibly earlier)

The one document the owner and the driving agent follow, top to bottom, at tonight's
at-desk bench. It orders the lane documents and links them for detail; it repeats only the
commands. Part of the original plan already ran unattended between 03:00 and 04:40 on the
owner's go of that night ("Done unattended", next); "The run" is what remains.

| Lane document | What it holds |
| ------------- | ------------- |
| [`docs/game-sessions.md`](../game-sessions.md), run sheets OW-1 and OW-2 | the run sheets, their predictions and how to read results (OW-2's corrections from the rehearsal are in block 3 below, not yet in the run sheet) |
| [`ow1-prep.md`](ow1-prep.md) | the session preflight and its decision tree (section 4) |
| [`verify-mod.md`](verify-mod.md) | the verification plugin, the 25-row re-run, the self-reporting quest, entering the results |
| [`input-results.md`](input-results.md) | what the input trial found and measured, and the executor recommendation |
| [`ck-sitting.md`](ck-sitting.md) | the CK batch, the first-minute checks, the CK after-step |
| `C:\Modding\kitkraft-observe\scripts\input\README.md` (outside the repo, not under version control) | the input executor and scenarios SC-1 to SC-5 (scripts fixed overnight; see `RESULTS-20261116.md` there) |
| `plan/adr/0010-agent-driven-game-sessions.md` Amendment 2, `plan/adr/0012-ck-interface-clean-room.md` | the rulings: input and focus allowed, a launch needs the owner's go; CK saves are allowed (ADR 0012 Amendment 3) |

Conventions: `$wt` is the worktree the bench runs from: one at `main` after this sheet's
branch has merged, named by the director (`C:\Modding\kitkraft-worktrees\bench-ck` is at
`2ef0c0cc` and lacks `rerun.toml`, the quest recipe and the `getav` fix: do not run from it);
`$kit = 'C:\Modding\kitkraft\.targets\x\release\kit.exe'`;
`$env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'`;
base save `KitKraftBase`, SHA-256
`fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf` (written out in full
below). Profile `Kitkraft Dev` only, never `Default`.

## Done unattended (2026-11-16, 03:00 to 04:40)

| Block of the original plan | Result | Where |
| -------------------------- | ------ | ----- |
| OW-1, send order (ran twice) | **inconclusive**: no line arrived late in either run; answers to commands sent while a `coc` or `load` was moving were **dropped, not delayed**. SO-03 is for the owner's reading (below) | `game-20261116-01`, `-02` (the reading covers both) |
| Verification mod (three sessions) | evidence entered from `game-20261116-04`: the gap matrix's game column is now **31 pass, 6 inconclusive** (presence only), save/reload **18 pass**; 20 `help` rows and V-C-AVIF stayed inconclusive (re-run below). PKIN answered in favour of the pack-in (KK-250) | `game-20261116-03` to `-05`, `docs/schema/evidence.toml`, `docs/foothold/gap-matrix.md` |
| Input experiment (in session `2026-11-16-T06`) | keyboard, mouse look and mouse buttons **work with the game focused**, **nothing works unfocused**; the built-in virtual pad is **invisible to the game**. Scenarios: container (SC-1) and weapon (SC-3) **pass**, NPC dialogue (SC-2) **partial**, workbench (SC-4) **inconclusive**, menu save (SC-5) **skipped** | [`input-results.md`](input-results.md), `game-20261116-06` |
| OW-2 | **not run**: a rehearsal by desk check only. The window could not be driven: the computer-use access request reaches a dialog only when it names the full path of `kit-ui.exe`, and the owner must click Allow | `F:\AI\Scripts\overnight\reports\ow2-rehearsal-20261116.md` (outside the repo); its fixes are in block 3 |
| CK batch | not run | - |

## Done unattended (2026-11-17, 00:24 to 00:47)

On the owner's go ("I'm going to bed you are go", about 00:20): A1 yes, A2 yes, A3 not
available (nobody to click Allow), A4 to A13 by the 2026-11-16 ruling. Release binaries as
below, not rebuilt; worktree `bench-night` at `d8c3b1c3`.

| Block | Result | Where |
| ----- | ------ | ----- |
| 0 Preflight, builds | `fails: 0`; base save and all plugin hashes as expected; new `VerifyReport.pex` `d366bded...e2c043`; the rename `probes.toml` is `3aa2d0cf...18ea2f` (KK-360's T-02 row, not a `kit` change). CK self-tests left to the CK lane | `game-20261117-01`, reading |
| 1 Verification re-run | **25 of 25 pass** (V-C-AVIF graded; 20 V-H rows on the reporter's lines). Evidence: 15 types new as presence-only `inconclusive`; AVIF's pass gains the entry | `game-20261117-01`, `docs/schema/evidence.toml` |
| 2 Verification quest | **3 pass, 1 fail**: Q-01, Q-02, Q-04 pass; **Q-03 fails** (the stage 10 fragment left no log line; stage 10 was set) | `game-20261117-02` |
| 3 OW-2 part 1, CLI parts only | first launch failed (MO2: "Cannot start sfse_loader.exe", from a Git Bash-started holder; nothing ran); retry: **T-02 `additem` pass** (first real run), T-01 and T-03 `inconclusive` (late or ambiguous replies), T-04 manual. Every window check (KK-032 to KK-036, KK-040) `not-run` (A3) | `game-20261117-03`, `-04` |
| 3 Input scenarios | **SC-4 pass** (retry with more pitch), **SC-2 pass** (Vasco, E), **SC-5 pass** (menu save in the dev profile) | [`input-results.md`](input-results.md), `game-20261117-04` |
| 4 OW-2 part 2 and 3, M6.O4 | not run: `KC_F31.esp` is made in the window (A3), and the sign-off is the owner's | - |
| 5 CK batch | run unattended 01:02 to 01:52 by the CK lane: 8 groups and 5 twins loaded and saved in place, F28 saved by the agent; ck-differential **pass** for 12 types (10 new), the rest inconclusive; the mods disabled again ([`ck-sitting.md`](ck-sitting.md), "Results, 2026-11-17") | `ck-20261117-11` to `-25`, `-31` |

The owner's `Documents\My Games\Starfield\Saves` is unchanged (listing compared before and
after). The game, MO2 and SFSE are closed; no `ABORT` file.

**For the owner's reading, SO-03** (`game-20261116-01`, "For the owner to look at"): two
`prid RussianHuntingRifle` sends, one failure line, and a `GetPos: Z` line between them.
If the failure line was the second send's own, nothing was out of order (the reading's
choice: every other echo-only send that night lost its answer). If it was the first send's,
late, the `Z` line came before an earlier request's line: the run sheet's contradiction.
The text cannot tell the two identical lines apart. Your call whether ruling 2 (send order)
stands.

## Release binaries

Verified by hashing the files at 05:00 on 2026-11-16 (built 04:55 from `e33e1c39`, `main`
then; it has the `getav` correlation the re-run needs and the quest fragment blocks):

| Binary | SHA-256 | Built |
| ------ | ------- | ----- |
| `C:\Modding\kitkraft\.targets\x\release\kit.exe` (package `kit-cli`) | `627a8a6d606e2010eee8adfc5e821713e0aac6842798e196d526fe464b0ae732` | 2026-11-16 07:54 from `c32becce` |
| `C:\Modding\kitkraft\.targets\x\release\kit-ui.exe` (package `kit`) | `250a52083469c3a2de206100ddce312e0d634a36bea609e5e37f5a18566401c2` | the same build |

The morning merge landed KK-360 only (an overridden inventory item gets an `additem` row after
its `help` row; the OW-2 Rename session therefore shows four rows, T-01 help, T-02 additem,
T-03 save/reload, T-04 manual, and T-02 has run only against the fake console), and the
components lane landed after it; the binaries above are built from that `main`. If `main`
moves again and the bench should run it, rebuild **before** the first session,
never while the game runs (about 2.5 minutes warm), from `$wt` at the commit to run, and
record the new hashes in the first ledger reading:

```powershell
$env:CARGO_TARGET_DIR = 'C:\Modding\kitkraft\.targets\x'; $env:CARGO_BUILD_JOBS = '4'; cargo build --release -p kit-cli -p kit; Get-FileHash -Algorithm SHA256 C:\Modding\kitkraft\.targets\x\release\kit.exe, C:\Modding\kitkraft\.targets\x\release\kit-ui.exe
```

Three of tonight's workarounds fall away with a rebuild that holds them: KK-368 (the window
finds the checkout above the working directory first, so `KITKRAFT_REPO_DIR` is no longer
needed), KK-367 (the CLI deploy preview names the plugin's SHA-256), KK-366 (the title stops
saying read-only).

## Before the bench (owner, 5 minutes)

1. Close Starfield, the CK and anything of SFSE.
2. Open MO2, select the profile **`Kitkraft Dev`**, then **close MO2**.
3. The base save `KitKraftBase.sfs` still hashed `fbed2cc2...eb5ccf` at 05:00, but the
   game wrote an autosave beside it during the input trial (notes below): the preflight
   re-hashes it (block 0), and a different hash stops the evening's sessions.
4. Load order now (after the unattended blocks): `KitKraftFirstPOI.esm` and
   `OldSovietMarksmanRifle.esm` enabled; `KitKraftVerify.esm` deployed, its lines present
   but disabled, **after** the rename plugin. That disabled line is why `deploy status`,
   the deploy preview and the Cockpit warn "plugin `OldSovietMarksmanRifle.esm` is not
   last" (block 3). The mini-site, scripts and hostile-fixture mods stay disabled.
5. Leave the primary monitor free: the CK and the game must open there (screenshots
   capture only the primary screen).

## Owner answers (one pass, before anything runs)

Last night's launch go and desk-free answers covered last night only; tonight's are asked
again. The agent records the answers verbatim in the first ledger reading and passes the
save words to `-OwnerSaveOk`.

**Settled by the owner's ruling of 2026-11-16** (ADR 0012 Amendment 3, ADR 0010 Amendment 3):
A4 to A13 are no longer asked. A4, A5 and A6 are **yes** (the words for `-OwnerSaveOk` are the
ruling's, quoted in ADR 0012 Amendment 3); A7 to A12 take the recommended answer; A13 is
**agent**. Only A1, A2 and A3 are asked at the bench: the launch go, the desk-free time and
the Allow click.

| # | Question | Pick | Recommended |
| - | -------- | ---- | ----------- |
| A1 | Game launch go for this evening: the verification re-run (one launch), the verification quest (one), OW-2 (two in part 1, one in part 2)? | yes / no | **yes** |
| A2 | The remaining input scenarios inside OW-2 part 1's session (SC-4 workbench, SC-2 dialogue opened by E), the agent taking the focus, desk free about 10 minutes? | yes / no | **yes** |
| A3 | Click **Allow** on the computer-use dialog when the agent asks for `C:\Modding\kitkraft\.targets\x\release\kit-ui.exe` (OW-2; that window only)? | yes / no | **yes** (without it OW-2 cannot run: the rehearsal was denied with no one at the desk) |
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

Dropped since last night: the gamepad step (the built-in virtual pad never reaches the
game; a driver-based pad is not proposed, [`input-results.md`](input-results.md)). Asked
later, not now: the M6.O4 sign-off (end of block 4), SO-03's reading (above), and the
executor design ([`input-results.md`](input-results.md), "Recommendation"). The reasons
behind A5 to A13 are in [`ck-sitting.md`](ck-sitting.md), "Owner answers needed".

## The run

Game sessions first, the CK batch last: its `deploy enable` puts 13 `KCB_*` plugins into
`Kitkraft Dev`, which every later game session would load (`KCB_F23k.esp` moves a reference
in the apartment cell the sessions use, `KCB_G8_Planet.esp` clones planet and star
records), and every runtime slot after them shifts. The two verification sessions go
first: agent-only, no window, the most evidence per minute; the re-run needs slot `0A`,
which holds only while nothing else is enabled.

| Block | What | Who | Minutes | Running total |
| ----- | ---- | --- | ------: | ------------: |
| - | Before the bench, owner answers | Owner | 10 | 0:10 |
| 0 | Preflight, builds (no game) | Agent | 10 | 0:20 |
| 1 | Verification re-run, 25 rows (`rerun.toml`, new `.pex`) | Agent | 12 | 0:32 |
| 2 | Verification quest (`KitKraftVerifyQuest.esm`, rows Q-01 to Q-04) | Agent | 12 | 0:44 |
| 3 | OW-2 part 1, the remaining input scenarios in its session | Agent drives `kit-ui`, owner clicks Allow and watches; desk free during input | 40 | 1:24 |
| 4 | OW-2 part 2 (`KC_F31.esp`) and part 3, M6.O4 sign-off | Agent drives `kit-ui`, owner watches | 45 | 2:09 |
| 5 | CK batch: deploy, G1 alone (owner watches), groups G2 to G8, disable | Agent; owner free after G1 | 50 | 2:59 |
| 6 | Close-out: evidence, `cargo xtask parity`, gate, commit | Agent (no game) | 30 | 3:29 |
| 5+ | Optional CK, in this order: fixture twins (25), F28 by the agent (8), F23, F20, F27 with the owner's hands (15 each), failed groups split by type (5 a type) | Agent, owner for F2x | up to 78 | up to 4:47 |

Starting at 21:30 the core ends about 01:00; with every optional CK step about 02:15.

**If only 60 minutes:** before and answers, block 0, block 1, block 2 (0:44), then block 5's
deploy, G1 with the owner watching, and the CK `deploy disable` (15): about **59 minutes**.
The two verification sessions are the cheapest evidence left (up to 20 presence rows and
the `getav` row, then the quest fragment), and G1 is the CK runner's first drive of the
real CK, which needs the owner's eyes once. Evidence entry and the gate run afterwards
without the owner.

**If only 2 hours:** the 60-minute cut with block 5 whole (the seven groups run while the
owner is away from the desk): about **1:34**, then block 6, which needs no owner (about
2:05 in all). Up to 56
new `ck-differential-tested` types beats OW-2, which changes one milestone row, needs the
owner at the desk for 85 minutes and the Allow click; OW-2 and the input scenarios move to
the next window together.

### Block 0: preflight and builds (no game, MO2 closed)

```powershell
cd $wt
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\bench\ow1-preflight.ps1 -Kit $kit
# the verification plugin and its new reporter (.pex only is new; no MO2 write):
& $kit papyrus compile docs\bench\verify-mod\scripts\KitKraft\VerifyReport.psc out\verify\Scripts --sources-zip "C:\Program Files (x86)\Steam\steamapps\common\Starfield\Tools\ContentResources.zip"
& $kit build docs\bench\verify-mod\verify-mod.toml out\verify\KitKraftVerify.esm --overwrite
& $kit package out\verify\KitKraftVerify.esm out\verify\pkg --scripts out\verify\Scripts\KitKraft\VerifyReport.pex --spec docs\bench\verify-mod\verify-mod.toml --plugins Starfield.esm --overwrite
# the verification quest:
& $kit papyrus compile docs\bench\verify-mod\scripts\KitKraft\VerifyQuestReport.psc out\verify-quest\Scripts --sources-zip "C:\Program Files (x86)\Steam\steamapps\common\Starfield\Tools\ContentResources.zip"
& $kit papyrus compile docs\bench\verify-mod\scripts\Fragments\Quests\QF_KKQ01Qust.psc out\verify-quest\Scripts --sources-zip "C:\Program Files (x86)\Steam\steamapps\common\Starfield\Tools\ContentResources.zip"
& $kit build docs\bench\verify-mod\verify-quest.toml out\verify-quest\KitKraftVerifyQuest.esm --overwrite
& $kit validate --plugin out\verify-quest\KitKraftVerifyQuest.esm --plugins Starfield.esm
& $kit roundtrip out\verify-quest\KitKraftVerifyQuest.esm --verify
& $kit package out\verify-quest\KitKraftVerifyQuest.esm out\verify-quest\pkg --scripts out\verify-quest\Scripts\KitKraft\VerifyQuestReport.pex --scripts out\verify-quest\Scripts\Fragments\Quests\QF_KKQ01Qust.pex --spec docs\bench\verify-mod\verify-quest.toml --plugins Starfield.esm --overwrite
# OW-2's inputs:
& $kit build docs\smoke-rename.toml out\OldSovietMarksmanRifle.esm --overwrite
& $kit testplan out\OldSovietMarksmanRifle.esm out\rename-plan --plugins Starfield.esm
# CK desk tests (launch nothing):
cd C:\Modding\kitkraft-observe\scripts; powershell -File .\ck-plugin.selftest.ps1; powershell -File .\ck-sitting.selftest.ps1
```

- **Pass:** preflight `fails: 0`, base save `fbed2cc2...eb5ccf`, `would start session ...`;
  `KitKraftVerify.esm` `a9632f11...e39faa` (unchanged; record the new `VerifyReport.pex`
  hash); `KitKraftVerifyQuest.esm` `94d2ad19...1dea3f6d`, validate 0 errors, roundtrip
  identical; rename plugin `9c0b28a6...78ea27`, rename `probes.toml` `d84e011a...a55dc05`;
  CK self-tests `0 failed`.
- **Failure:** [`ow1-prep.md`](ow1-prep.md) section 4 for every preflight rule. A plugin hash
  that differs means `kit` changed: stop and tell the director before any session. A base
  save hash that differs: stop the evening's sessions (the base save is the owner's).
- **After:** paste the preflight's list-file hashes into the notes (the evening's "before").

### Block 1: the verification re-run (25 rows)

Detail: [`verify-mod.md`](verify-mod.md), "Re-run of the corrected rows". The deploy
replaces one file (the `.pex`); enable sets the plugin's two lines back and moves it last
(slot `0A`, which `rerun.toml` assumes).

```powershell
cd $wt
& $kit deploy out\verify\pkg\KitKraftVerify C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev" --dry-run   # then without
& $kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftVerify --dev --profile "Kitkraft Dev" --dry-run   # then without
& $kit game session C:\Modding\MO2-Starfield docs\bench\verify-mod\rerun.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger $wt\docs\game-observations.md --dry-run
# no refusal: the same without --dry-run; hands off, about 6 minutes (100 s to the base load, 3 minutes of rows)
```

- **Pass:** V-01, V-40, V-P00, V-P99 pass (the reporter ran); V-C-AVIF passes (the `getav`
  correlation); each V-H row passes on `KKV-H<nn> <TYPE> present=True`.
- **Failure:** a V-H row that fails says the form did not load: a real finding, enter it
  `fail`. Launch or base-load timeouts: [`ow1-prep.md`](ow1-prep.md) section 4.
- **After (during block 2's session):** enter a passing V-H row as the six presence rows of
  `game-20261116-04` were (`game-runtime-tested`, `inconclusive`, "Presence only"). Leave
  `KitKraftVerify` enabled for block 2 (the quest goes after it).

### Block 2: the verification quest (4 rows)

Detail: [`verify-mod.md`](verify-mod.md), "Self-reporting quest". The quest starts when the
base save loads, so its lines are in the Papyrus log before Q-01 reads it; no row needs a
runtime slot.

```powershell
cd $wt
& $kit deploy out\verify-quest\pkg\KitKraftVerifyQuest C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev" --dry-run   # then without
& $kit deploy enable C:\Modding\MO2-Starfield --mods KitKraftVerifyQuest --dev --profile "Kitkraft Dev" --dry-run   # then without
& $kit game session C:\Modding\MO2-Starfield docs\bench\verify-mod\verify-quest-probes.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger $wt\docs\game-observations.md --dry-run
# no refusal: the same without --dry-run; hands off, about 4 minutes
& $kit deploy disable C:\Modding\MO2-Starfield --mods KitKraftVerifyQuest --mods KitKraftVerify --dev --profile "Kitkraft Dev" --dry-run   # then without
```

- **Pass:** Q-01 to Q-04: the quest script ran (Q-01, Q-04), its object property held the
  reference the same build placed (Q-02), the stage 10 fragment ran (Q-03).
- **Failure:** Q-03 failing alone points at the fragment block; read the Papyrus log in the
  session's `logs\`. A quest that starts once per save: these rows read the base save's
  first load only, so one session is the test.
- **After:** the writer's reading names the two `.pex` hashes and the plugin's.

### Block 3: OW-2 part 1, with the remaining input scenarios in its session

Run sheet: [`game-sessions.md`](../game-sessions.md), "Run sheet OW-2", Part 1, steps 1.1
to 1.11, with the rehearsal's corrections (they are not in the run sheet yet):

- **Before 1.1, in the terminal that starts the window:**
  `$env:KITKRAFT_REPO_DIR = $wt` (the window otherwise takes its checkout from above
  `kit-ui.exe`, which is the main checkout `C:\Modding\kitkraft`, whatever folder `kit ui`
  runs from; KK-368 changes that). Then `& $kit ui --dev` from `$wt`. A Windows Terminal
  window for `kit.exe` opens beside the workbench; expect it.
- **Computer use:** the agent's access request must name the full path
  `C:\Modding\kitkraft\.targets\x\release\kit-ui.exe` (the name `kit-ui`, the title or
  "KitKraft Workbench" resolve to nothing), and the owner clicks **Allow** (A3). The title
  bar reads "KitKraft Workbench (read-only)" although the window writes (KK-366); ignore it.
- **1.3, Status and Preview deploy:** expect the warning "plugin
  `OldSovietMarksmanRifle.esm` is not last": the disabled `KitKraftVerify.esm` and
  `KitKraftVerifyQuest.esm` lines sit after it (a disabled line loads nothing; the warning
  is expected and changes no grade). To clear it instead, before 1.1, with MO2 and the game
  closed:
  `& $kit deploy enable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev" --dry-run`
  (expect "would edit", its `plugins.txt` and `loadorder.txt` lines moved last), then
  without `--dry-run`. The preview's plugin SHA-256 shows in the window only; the CLI's
  text form prints `1 unchanged` and no hash (KK-367). 1.4's confirmed deploy is a no-op.
- **1.5, the session form:** type `C:\Modding\kitkraft-observe` into the observe field (the
  form fills `<checkout>\observe`), and check that the ledger field is
  `$wt\docs\game-observations.md`.
- **Input scenarios** (A2 yes), after 1.7 (phase idle), game in front, and only while the
  panel's elapsed time is under 1,400 s (the session's budget is 1,800 s):

  ```powershell
  cd C:\Modding\kitkraft-observe\scripts\input
  python kkconsole.py getpos                                         # three numbers: the console answers
  python run_scenarios.py --only SC-4 --ask                           # workbench: E opens the menu?
  python run_scenarios.py --only SC-2 --ask --param npc_ref=000057BE  # dialogue opened by E
  # SC-5 only with A4 yes: python run_scenarios.py --only SC-5 --ask --allow-menu-save
  ```

  - **SC-4:** last night the "Use (E)" prompt showed but E returned to the world. Stand the
    player on open floor facing the bench before E; a screenshot decides.
  - **SC-2:** last night `player.moveto 0000598D` (Noel) landed in a quest scene the base
    save started by itself, so E only chose an option. Tonight `VascoREF` (`000057BE`, same
    cell, `CityNewAtlantisLodgeInt`); if a scene starts by itself again, pick an idle NPC at
    the bench (`coc CityNewAtlantisLodgeInt`, click one in the console, pass its ref ID).
  - **Never press Tab blind:** it opens the Data menu, and that makes the game autosave
    into the dev profile's saves (notes).
- Then 1.8 to 1.11 (screenshot re-run, Record in Ledger, Stop, relaunch).

- **Pass (OW-2 part 1):** a ledger entry started from the window in `$wt`'s ledger, six
  statuses filled (`ck_open`, `ck_reexport` `not-run`), the screenshot shown, the manual row
  recorded.
- **Failure:** a stuck key: `python kkinput.py release`, or tap the key. If the budget runs
  out mid-input, the game ends and the executor's guard stops at its next 10 ms check; go
  to 1.11 for a fresh session. A ledger entry that lands in `C:\Modding\kitkraft\docs\`: the
  repo variable was not set; stop, tell the director, and do not commit there.
- **After (same night):** the writer's reading names the input run folders
  (`runs\<stamp>\`, `inputs.jsonl`): the controller's `report.json` does not hold sent
  input until KK-362.

### Block 4: OW-2 part 2 and part 3, the M6.O4 sign-off

Run sheet: [`game-sessions.md`](../game-sessions.md), "Run sheet OW-2", Part 2 (2.1 to
2.13) and Part 3; the repo variable and the observe field as in block 3 (2.10).

- Part 2 makes `out\f31\KC_F31.esp` in the window (set, clone, place), then validate,
  roundtrip, testplan, deploy, `deploy enable --mods KC_F31` (dry run first), one session
  from the window (2.11, 2.12), and `deploy disable --mods KC_F31` (2.13).
- Part 3: one note per check in the writer's reading.
- Then offer the owner M6.O4's sign-off (revised T3, `docs/design/workbench-performance.md`);
  his words verbatim into the reading.
- Last: `& $kit deploy disable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev" --dry-run`, then without.
- **Pass:** `KC_F31.esp` SHA-256 recorded; `parse`, `byte_roundtrip`, `game_runtime` filled;
  `save_reload` by 2.12.
- **Failure:** OK disabled in 2.2: record why and stop part 2 (`blocked`). Session faults:
  [`ow1-prep.md`](ow1-prep.md) section 4.
- **After:** the two readings; `KC_F31.esp`'s hash into the reading for the CK lane.

### Block 5: the CK batch (game closed for the rest of the evening)

Run sheet: [`ck-sitting.md`](ck-sitting.md), "Run sheet", steps 2 to 12. Needs A5 (saves)
and, for the fixtures, A6 to A13.

```powershell
$m = 'C:\Modding\kitkraft-observe\ck-2026-11-16\plugins\manifest.json'
# 1. each `kit deploy ... --dev --dry-run` line of C:\Modding\kitkraft-observe\ck-2026-11-16\plugins\deploy.txt, then without;
#    then its `kit deploy enable` line (one --mods per mod: the CLI takes one value per flag), --dry-run, then without
cd C:\Modding\kitkraft-observe\scripts
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 11 -DryRun     # every plugin "deployed and enabled"
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 11 -Only KCB_G1_Core.esp -Save -OwnerSaveOk "<A5 words>, 2026-11-16"   # first plugin alone; owner watches
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
python docs\bench\verify-mod\propose_evidence.py C:\Modding\kitkraft-observe\<session>\report.json   # blocks 1 and 2
cargo test -p kitkraft-engine --lib every_evidence_pointer_resolves
cargo xtask parity
cargo xtask ci
git add docs\game-observations.md docs\ck-observations.md docs\schema\evidence.toml docs\record-parity.md docs\foothold\gap-matrix.md
git commit   # never --no-verify; the gate passed on this commit
```

The hand-off lists each block's result in AGENTS.md's reporting block.

## Notes

- **Autosave in the dev profile (side effect of the input trial).** Saves of `Kitkraft Dev`
  are profile-local: `C:\Modding\MO2-Starfield\profiles\Kitkraft Dev\saves\` (beside
  `KitKraftBase.sfs` and the `KitKraftSession-*` saves). When Tab opened the Data menu at
  04:07 the game wrote `Autosave1_20261116M546564_000038_20261116090732_1_0_4.sfs` there.
  It is not a `KitKraftSession-*` save, so it is outside what a session may write. Nothing
  was deleted and nothing should be by an agent: removing it is the owner's call. The base
  save still hashed `fbed2cc2...eb5ccf` at 05:00; block 0's preflight re-hashes it.
- **Ledger id collision, fixed.** The input trial's session wrote its entry as
  `game-20261116-01` into the old `bench-ck` worktree, where no 2026-11-16 entry existed;
  `main` already had OW-1 under that id. The entry is now `game-20261116-06` here and the
  `bench-ck` copy was reverted. Run every session tonight with `--ledger` (or the window's
  repo) pointing at `$wt`, a checkout that already holds `-01` to `-06`.
- **`deploy enable --mods`** takes one value per flag (repeat `--mods`); `kit help` shows
  `--mods <mods>...`, which reads as several values after one flag (KK-365).

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

- **Never in a real session:** the timeout diagnostics (`stuck-<n>.png`); the launch wait's
  send-order count, which times out if the game drops one launch poll's answer, as it
  dropped busy-time answers all last night (KK-369; the 75 s settle makes it unlikely, not
  impossible; [`ow1-prep.md`](ow1-prep.md) section 1).
- **Send order:** OW-1 saw drops, never a late line; whether the console can reorder at all
  rests on SO-03's reading (above). Rows should wait out every move before sending what
  they grade.
- **Verification mod:** the reporter's presence lines (`KKV-H<nn>`) have never run; the
  quest's fragment block has never run in game.
- **The CK runner has never driven the real CK:** posted `BM_CLICK` on "Set as Active File";
  whether `EditorWarnings.txt` is written during the load; what the CK does with
  EditorID-only `[[new]]` records of types with required members; G8's planet records.
- **Input:** opening the workbench menu and NPC dialogue by E (SC-4, SC-2); the Data menu's
  autosave on every Tab is assumed, seen once.
- **The window:** OW-2's real-window checks (KK-032 to KK-036, KK-040) have run only against
  fakes; the input part shares part 1's 1,800 s budget.
- **MO2:** rewrites `ModOrganizer.ini` and `plugins.txt` at launch (known, restored by the
  controller, no grade change); a dialog "waiting on an application to close before
  exiting" may sit over the game for the first minutes and goes away by itself.

## Last night's dry runs (2026-11-15, 23:08 to 23:12)

With the release `kit.exe` of that night (`0bbcfc6d...317172`, from `a618e7d6`); MO2 was
open with `Default` selected, so every refusal was that, or a plugin not deployed by the
lane rules. They found one fix that holds tonight: `deploy.txt`'s enable and disable lines
named all mods after one `--mods`, which the CLI refuses; it takes one value per `--mods`,
and `deploy.txt` and `build.ps1` were corrected (original kept as
`deploy.txt.bak-20261115`). The full table is in git history (`2ef0c0cc`).
