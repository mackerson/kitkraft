# CK bench sitting, 2026-11-16 (from about 21:30)

The run sheet for the Creation Kit part of the bench evening. Goal: fill the `ck_open` and
`ck_reexport` columns for as many typed-writable record types as one evening allows, and
save the four CK fixtures that need the owner's hands (F28, F23, F20, F27; from
`plan/owner-sitting-2026-11-06.md`). Today 2 of 185 record types are CK-differential-tested
(CELL and REFR, `docs/schema/evidence.toml`); the batch below can take that to 58.

Prepared 2026-11-15 without starting the CK, the game, SFSE or MO2. Nothing here has run
against the CK yet; what was tested tonight, and how, is in "Tested on 2026-11-15".

## Rulings that bind this sitting

- `plan/adr/0012-ck-interface-clean-room.md`: the CK is a black box; no CK command-line
  switch (the only launch is MO2's own shortcut, `moshortcut://:Creation Kit`); no static
  scan of `CreationKit.exe`; a save in the CK only where the owner allowed it for this
  sitting (Amendment 2, decision 6).
- `plan/adr/0010-agent-driven-game-sessions.md` Amendment 2 (2026-11-15): the sitting may
  take window focus and type into the CK's file dialog. Every key, click and button message
  the scripts send is logged as `input: ...` in the sitting's `session.md`.
- Profile `Kitkraft Dev` only; MO2 writes only through `kit deploy --dev` and
  `kit deploy enable` / `disable` (ADR 0009); the CK's two settings files are backed up and
  restored by the frame (ADR 0012 Amendment 2, decision 4).
- Records stay in `C:\Modding\kitkraft-observe\` (sprint ruling D9); the repo gets counts,
  ledger entries and evidence pointers, never CK text beyond labels.

## Owner answers needed (before or at the bench)

### The save question

**May the agent save in the CK tonight?** Two separate yes/no answers:

1. **Bench batch:** each `KCB_*.esp` opened as the active file and saved in place
   (File > Save Plugin, no dialog; MO2 writes it into that plugin's own
   `KitKraft Dev - KCB_*` mod, nothing else). Recommended: **yes**. Without it the batch
   still runs and fills `ck_open`; `ck_reexport` stays `not-run`.
2. **F28 by the agent:** the coffee maker duplicated and saved as `KC_F28.esp` into
   `overwrite\` (`steps-F28.ps1`; the file name goes in by window message, tested tonight).
   Recommended: **yes**; F28 was OK'd before (Q3) and stopped only at the file box.

The scripts are built for both answers: `ck-batch.ps1` without `-Save` is observation only,
and the postflight fails a run in which any plugin file changed. With `-Save` the owner's
words go into `-OwnerSaveOk` and into every `result.json`.

### The seven fixture choices (from `plan/owner-sitting-2026-11-06.md`)

| # | Question | Recommended answer | Why |
| - | -------- | ------------------ | --- |
| 1 | F23: which reference, and is Position X = `1234.5` the move? | `Starfield.esm:1B21E0`, X = `1234.5` | A temporary REFR (wall `ComIntParSmWallA_MidFull01`, no EditorID) at (-17, -6, 4.63), right behind the F29 crate at (-17, -4), so it is easy to find in the Render Window (`kit cell --cell Starfield.esm:21966F`). KitKraft's twin `KCB_F23k.esp` (same edit) is built. The sentinel's bytes `00 50 9A 44` are easy to find in the diff |
| 2 | F20: `Crate_MilitarySupply_01` as the STAT and `KC_F20_Cell` as the EditorID? | Yes, both | Dragging this crate into the Render Window is observed (F29); `KCB_F20k.esp` already makes a cell named `KC_F20_Cell` |
| 3 | F27: Freestar Collective Embassy and `PlayerFaction`? | Yes | The same edit as `docs/recipes/ownership.toml` and `KCB_F27k.esp`, so the CK's and KitKraft's files compare member for member (M4.E8) |
| 4 | Part B first, or after the four saves? | After, and only if time remains | The bench batch's first launch now checks the frame before any save; the owner's hands are needed most for F23, F20 and F27 (never-observed menu paths) |
| 5 | B10 (the 12 submenus), if time allows? | Only if time remains, last | Optional, not in decision 12, and readable later from screenshots |
| 6 | Copy each saved fixture into `C:\Modding\kitkraft\observe\<yyyy-mm-dd>-<id>\` too? | No: keep everything in `C:\Modding\kitkraft-observe\` | `ck-ingest.ps1` and `cargo xtask oracle ck` read any folder; one copy, under ruling D9 |
| 7 | Do you delete the files in `overwrite\` after each launch, or may the agent? | You delete them, as with F29 | AGENTS.md lets agents write under MO2 only through `kit deploy`; it takes a few seconds per launch. The batch's in-place saves land in the dev mods, not `overwrite\` |

## What one automated run does (`ck-plugin.ps1`)

One plugin, one CK launch, no owner hands:

1. **Checks** (read only): the plugin is in its `KitKraft Dev - <stem>` mod with the
   built file's SHA-256, and the mod and plugin are enabled in `Kitkraft Dev`; otherwise it
   refuses and prints the `kit deploy` command that fixes it.
2. **Frame** (`ck-sitting.ps1`, unchanged): preflight (nothing running, the dev profile, the
   CK's SHA-256, list files copied), settings backup, launch through MO2.
3. **File > Data**: the dialog's controls are discovered at run time (UI Automation names
   first, Win32 control ids second, written to `controls-data.json`); the plugin row is
   selected by type-ahead (observed), checked by the "Created By" box (it must not read
   Bethesda's name), then **Set as Active File** and **OK** (observed, `ck-20261101-02`).
4. **Load**: waits until the title names the plugin as the active file (`- [<plugin>]`),
   at most 420 s (observed 80 to 180 s). A crash is `ck_open fail`; a prompt that holds the
   load 45 s, a title without the plugin, or the timeout is `inconclusive`; prompts are
   captured and never answered.
5. **Warnings**: the CK's `EditorWarnings.txt` and `EditorDataFilesLoaded.txt` (written by
   the CK in the game folder on every run) are copied into the sitting folder after the load
   and again after the exit; rows naming the plugin or one of its EditorIDs are counted and
   kept in `editor-warnings-*-plugin.txt`. Any such row turns `ck_open` into `inconclusive`
   until it is classed by hand.
6. **Save** (only with `-Save`): File > Save Plugin; the in-place save is detected by the
   dev mod file's hash or time (or a file of that name in `overwrite\`); `ck.esp` is
   copied out beside `kitkraft.esp` (the input as deployed). An unexpected dialog is
   captured, cancelled and stops the run (in place is the only save the run makes).
7. **End**: the frame closes the CK (stopped after 90 s if it hangs), closes MO2, restores
   the settings, runs the postflight (with `-Save`, exactly this plugin's file may change).

Each stage takes a screenshot at its start and end (`shots\<nn>-<stage>-start.png`, `-end`)
and rewrites `result.json`: stage times and statuses, controls found, prompts, title and
version, load seconds, warning counts, the saved file's hash, `ck_open`, `ck_reexport`
(`pending-diff` until the after-step), reasons, postflight.

`ck-batch.ps1` runs a list of plugins from the build's `manifest.json`, each in its own
PowerShell process with its own sitting id, with a per-plugin timeout (15 min; on timeout
the process is killed and `ck-cleanup.ps1` closes the CK and MO2, restores the settings and
runs the postflight). It stops before the next plugin on a stop file, a failed postflight,
or two launches in a row that did not bring the CK up. It writes `batch.json`, `batch.md`
and one `.out` per plugin.

## The batch

Built tonight from the recipes in `docs/bench/ck-batch/` with `build.ps1` into
`C:\Modding\kitkraft-observe\ck-2026-11-16\plugins\` (`manifest.json`, `manifest.md` with
every SHA-256, `deploy.txt`). Every plugin passed `kit validate` (0 errors, 0 warnings) and
`kit roundtrip --verify` (identical); all are `.esp` with TES4 flags 0 (the CK takes only a
non-master file as its active file, `ck-20261101-01`).

- **Per type** (`types/<TYPE>.toml`, 58 files, one per typed-writable type): one `[[clone]]`
  of a vanilla record (the form and why it was picked is in each file's comment) and one
  minimal `[[new]]` (EditorID only); CELL is a new interior cell, REFR the F29 crate placed
  persistent and temporary, ACHR a vanilla NPC placed where the actors recipe places one,
  DIAL and INFO new records under a new quest as the dialogue recipe.
- **Per group** (`groups.txt`, 8 plugins `KCB_G1_Core` to `KCB_G8_Planet`): the type
  bodies joined. These are what the batch runs by default.
- **Fixture twins** (`fixtures/`, 5 plugins): KitKraft's side of F20 (the cell only:
  `build` cannot place into a cell it makes in the same spec), F23 (1B21E0's X = 1234.5),
  F27 (the ownership recipe's edit), F28 (`clone` of the coffee maker), and F31k, a spec-built
  stand-in for the M7 exit plugin (one set, one clone, one place): `KC_F31.esp` from OW-2 does
  not exist yet (checked 2026-11-15).

**Why one plugin per group, with per-type plugins as the fallback.** A CK launch costs about
4 to 6 minutes, nearly all of it loading `Starfield.esm` (115 to 134 s observed) plus MO2's
start and stop; only one active file is possible per launch, so 58 per-type launches would
take 4 to 6 hours. Eight group launches take about 40 minutes and cover all 58 types. If a
group plugin crashes the CK or comes back with differences that hide which type caused them,
only that group's types are rerun one by one (`-Kinds type -Only KCB_T_...`), so a crash
costs one group, not the evening. The fixture twins run after the groups.

## Run sheet

Sitting ids: `ck-20261116-01` to `-05` stay free for the owner sitting's launches (Part B,
F28, F23, F20, F27, in `plan/owner-sitting-2026-11-06.md`); the bench batch starts at
`-11`. Times are estimates from the 2026-11-01 to -03 sittings.

| Step | What | Who | Time |
| ---- | ---- | --- | ---- |
| 0 | Owner answers the save question and the seven choices above | Owner | 5 min |
| 1 | Owner closes Starfield, SFSE, the CK; in MO2 selects the profile `Kitkraft Dev` (tonight's `kit deploy --dry-run` refused only because MO2's selected profile is `Default`), then closes MO2 | Owner | 2 min |
| 2 | `powershell -File C:\Modding\kitkraft-observe\scripts\ck-plugin.selftest.ps1` and `ck-sitting.selftest.ps1` (desk tests, launch nothing) | Agent | 2 min |
| 3 | Deploy: each `kit deploy ... --dev --dry-run` line of `plugins\deploy.txt`, read, then without `--dry-run`; then the `kit deploy enable ... --dry-run` line, read, then without | Agent | 3 min |
| 4 | `ck-batch.ps1 ... -DryRun` (below): every plugin "deployed and enabled" | Agent | 1 min |
| 5 | **First plugin alone** with the owner watching: `ck-batch.ps1 ... -Only KCB_G1_Core.esp` (with `-Save` if allowed); the first-minute confirmations below | Agent, Owner watches | 6 min |
| 6 | The other 7 groups: `ck-batch.ps1 ... -StartNn 12 -Only KCB_G2_Items.esp,...` (or all groups but G1). Owner is free (the agent holds the desktop) | Agent | 35 to 45 min |
| 7 | After-step for each saved plugin as it lands (below), while the next one runs | Agent | 2 min each |
| 8 | Fixture twins: `-Kinds fixture` | Agent | 25 min |
| 9 | F28 by the agent (if allowed): `ck-run.ps1 -Id ck-20261116-02 -Name F28 -Steps .\steps-F28.ps1 -SaveAllowed F28` | Agent | 8 min |
| 10 | F23, F20, F27 with the owner's hands: `ck-run.ps1 ... -Steps .\steps-owner-fixture.ps1 -SaveAllowed F2x` per `plan/owner-sitting-2026-11-06.md` A2 to A4 (owner deletes `overwrite\KC_F2x.esp` and `Backup\AutoSaveDEFAULT.bak` after each) | Owner + Agent | 15 min each |
| 11 | Any group that failed: its types one by one (`-Kinds type -Only KCB_T_A.esp,KCB_T_B.esp`) | Agent | 5 min per type |
| 12 | After: `kit deploy disable ... --dry-run`, then without (the line in `deploy.txt`); owner switches MO2 back to `Default` before playing | Agent, Owner | 3 min |

Commands (from `C:\Modding\kitkraft-observe\scripts\`; `$m` is the manifest):

```powershell
$env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
$m = 'C:\Modding\kitkraft-observe\ck-2026-11-16\plugins\manifest.json'
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 11 -DryRun
# with the owner's save yes (his words, verbatim):
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 11 -Only KCB_G1_Core.esp -Save -OwnerSaveOk "<owner's words>, 2026-11-16"
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 12 -Only KCB_G2_Items.esp,KCB_G3_Weapons.esp,KCB_G4_Objects.esp,KCB_G5_World.esp,KCB_G6_Actors.esp,KCB_G7_Quest.esp,KCB_G8_Planet.esp -Save -OwnerSaveOk "<same words>"
powershell -File .\ck-batch.ps1 -Manifest $m -Date 20261116 -StartNn 21 -Kinds fixture -Save -OwnerSaveOk "<same words>"
# without a save: the same lines without -Save and -OwnerSaveOk (ck_open only)
```

A rerun of a saved plugin needs a redeploy first (the CK's save changed the dev mod's
file; the run refuses and prints the command): `kit deploy <plugin> C:\Modding\MO2-Starfield --dev`.

## The first minute at the bench (first plugin, owner watching)

Confirm, and write each answer into the run's `session.md`:

1. The CK window opens on the **primary monitor** (screenshots capture the primary screen
   only) and nothing else of the owner's covers it.
2. `controls-data.json`: the Data dialog's list, "Set as Active File" and OK were found
   (`result.json` `controls`), and "Created By" read something other than Bethesda's name
   once `KCB_G1_Core.esp` was selected.
3. `data-03-active.png` shows `KCB_G1_Core.esp` as "Active File" in the list (the status
   column is not exposed to UI Automation; the screenshot is the check).
4. The title after the load reads `... - [KCB_G1_Core.esp] ...` and the version is
   1.16.244.0; the load time (`load_seconds`) is in the 80 to 180 s range.
5. `editor-warnings-loaded.txt` exists (the CK wrote `EditorWarnings.txt` during the
   load, not only at exit); if it does not, `-exit` is the record and that is fine.
6. With a save: File > Save Plugin opened **no** dialog and `result.json` `saved.how` is
   `in place (dev mod)` (as `ck-20261101-02`); without: the postflight says nothing changed.
7. The CK closed with no prompt; MO2 closed; the postflight is `ok`.

If any of 2 to 6 differs, stop the batch (stop file), read the screenshots together, and
decide before the next plugin.

## Abort

- **Clean stop** (finishes the running plugin's clean-up, then stops):
  `New-Item C:\Modding\kitkraft-observe\ck-2026-11-16\STOP`. Remove it before the next batch
  (the batch refuses to start while it exists).
- **The owner needs the desktop now**: switching windows is enough; the scripts' input
  stops by itself when no CK window is in front (`STOP: CK not in front`), and the frame
  closes the CK (unsaved changes are discarded) and restores everything.
- **Hard abort** (a script hangs): Ctrl+C in the batch window, then
  `powershell -File .\ck-cleanup.ps1 -SittingDir C:\Modding\kitkraft-observe\ck-2026-11-16\<sitting folder>`
  (add `-SaveAllowed <plugin>.esp` for a saving run). It closes the CK (stopped after
  90 s) and MO2, restores the two settings files and runs the postflight.
- After any abort: `Get-Process Starfield, sfse_loader, CreationKit, ModOrganizer` prints
  nothing, and the postflight line in `session.md` is `ok` before anything else starts.

## After each plugin: diff and evidence, the same night

From the worktree (or the main checkout once merged), with `kit.exe` built from it:

```powershell
$env:KITKRAFT_DATA_DIR = 'C:\Program Files (x86)\Steam\steamapps\common\Starfield\Data'
powershell -File docs\bench\ck-batch\ck-ingest.ps1 -Sitting C:\Modding\kitkraft-observe\ck-2026-11-16\bench-11-KCB_G1_Core -Manifest $m -Kit <kit.exe>
# read <sitting>\ingest\verdicts.json, evidence-entries.toml, ledger-entry.md; then:
powershell -File docs\bench\ck-batch\ck-ingest.ps1 ... -Apply
cargo test -p kitkraft-engine every_evidence_pointer_resolves
cargo xtask parity
```

What it does: copies `kitkraft.esp` and `ck.esp` into a fixture folder with a
`fixture.toml` (every record KitKraft made listed under `match`; the three TES4
normalizations M5.A3 classed, citing `ck-20261101-02`), runs `cargo xtask oracle ck` on it,
and reads its report. A type's `ck_reexport` is `pass` when the CK's file round-trips and the
oracle has no unexplained or ours-missing item for a record of that type; otherwise
`inconclusive` (classed by hand later, M5.A3), never `pass`. The evidence entry is `pass`
only when `ck_open` and `ck_reexport` both are. `-Apply` appends the ledger entry to
`docs/ck-observations.md` and the entries to `docs/schema/evidence.toml` (a pointer is added
to an existing entry of the same type and status, as CELL's and REFR's); then the gate test
above checks every pointer resolves and `cargo xtask parity` regenerates the counts.

For a CK fixture (F20, F23, F27, F28) the twin is the comparison:
`ck-ingest.ps1 -Ours <plugins>\fixtures\KCB_F23k.esp -Ck <sitting>\KC_F23.esp -SittingId <id> -Types REFR`
gives the drafts the same way; the matrix's first questions for those fixtures are read by
hand from the oracle's report.

## What a full pass would add

| Column | Today | After a full pass, all clean |
| ------ | ----: | ---------------------------: |
| `ck-differential-tested` pass | 2 (CELL, REFR) | 58: every typed-writable type, 56 new |
| `ck_open` observed on a KitKraft plugin | 2 types (F30's) | 58 types plus the five twins |
| `ck_reexport` observed | 2 types | 58 types (with the save yes) |
| CK fixtures saved | 0 of F20, F23, F27, F28 | 4 (F28 by the agent, three with the owner) |

Without the save: `ck_open` for 58 types, `ck-differential-tested` unchanged (an open
without a re-export is not a differential test). Types that come back with differences stay
`inconclusive` until classed; that is still evidence the matrix lacks today.

## Results, 2026-11-17 (unattended, 01:02 to 01:52)

Run by the CK lane with nobody at the desk, saves under ADR 0012 Amendment 3 (its words in
`-OwnerSaveOk`), from `C:\Modding\kitkraft-worktrees\bench-night`; `kit.exe` the release
binary of "Release binaries" in [`README.md`](README.md), not rebuilt. Sitting ids
`ck-20261117-11` to `-18` (groups), `-21` to `-25` (twins), `-31` (F28); records in
`C:\Modding\kitkraft-observe\ck-2026-11-17\`; ledger `docs/ck-observations.md`. Step 2's
self-tests: 0 failed (both). Deploy (step 3): 13 plugins with `kit deploy --dev`, enabled, and
disabled again after the sitting (step 12). Every launch: the plugin became the active file,
loaded in 105 to 125 s (F20k alone in 15 s: it names no master), File > Save Plugin saved in
place with no dialog, the CK closed with no prompt, postflight `ok`. The first-minute checks
(G1, nobody watching): 2 to 7 held; 1 (primary monitor) is the screenshots' evidence.

| Plugin | Id | ck_open | Saved | ck_reexport pass | Inconclusive (why, in short) |
| ------ | -- | ------- | ----- | ---------------- | ---------------------------- |
| `KCB_G1_Core` | `-11` | pass | in place | GLOB, TXST, SOUN, WWED, IRES | KYWD, FLST, MESG, RSPJ, AVIF (the new record gained members) |
| `KCB_G2_Items` | `-12` | pass | in place | - | all 9 (BOOK `CNAM` width, padding, new records filled) |
| `KCB_G3_Weapons` | `-13` | pass | in place | - | all 5 (COBJ padding, new records filled, WEAP speeds reset) |
| `KCB_G4_Objects` | `-14` | pass | in place | TMLM | 10 (new records filled; clones all unchanged) |
| `KCB_G5_World` | `-15` | pass | in place | REFR | LCTN (clone's reference lists dropped), REGN (point order), CELL (new cell EditorID), ACHR (`XLRL`) |
| `KCB_G6_Actors` | `-16` | pass | in place | FACT | NPC_, PACK, PERK, LVLN, LVLP (padding, new records filled) |
| `KCB_G7_Quest` | `-17` | pass | in place | - | QUST (`VMAD`), DIAL (branches made, category), INFO (`TRDA`) |
| `KCB_G8_Planet` | `-18` | pass | in place | PCMT | 8 (STDT system ID reassigned, new records filled) |
| `KCB_F20k` | `-21` | pass | in place | - | CELL (EditorID without underscores, lighting added) |
| `KCB_F23k` | `-22` | pass | in place | CELL, REFR | - |
| `KCB_F27k` | `-23` | pass | in place | - | CELL (`XOWN` padding only) |
| `KCB_F28k` | `-24` | pass | in place | STAT | - |
| `KCB_F31k` | `-25` | pass | in place | WEAP, CELL, REFR | - |
| F28 (agent) | `-31` | - | `KC_F28.esp` into `overwrite\` (copied out, then removed) | - | against the twin: `EDID` and `OBND` (the CK's duplicate has zero bounds) |

`ck-differential-tested` pass: 2 -> 12 types (`docs/schema/evidence.toml`, `cargo xtask
parity`). Every CK save also drops TES4 `INCC`. Not run: the per-type fallback (the oracle
pins every difference to one record, so no group needed splitting); F20, F23, F27 by the CK
(no automated steps; they need the owner's hands, step 10); F30 and F29 again (done
2026-11-01); Part B and B10. The two settings files hash as before the sitting; `overwrite\` is
empty; the owner's `Saves` listing is unchanged.

## Tested on 2026-11-15 (no CK, game, SFSE or MO2 started; no input to the desktop)

- `ck-plugin.selftest.ps1`: 52 checks, 0 failed (62 with `-SaveDialog`). Script parsing; plugin-name and escaping
  rules; the postflight's exact-file save rule; warning counting on made-up rows and on the
  2026-11-01 copy (1,590 rows, none naming the POI plugin, as the ledger says); deploy
  checks against a fake MO2; stages, `result.json` and stop files; control lookup order;
  `ck-plugin.ps1` refusals; `ck-batch.ps1` against a stub plugin script (results collected,
  a hung run killed after its timeout and cleaned up, a failed postflight stopping the
  batch, a stop file refusing a start).
- `ck-plugin.selftest.ps1 -SaveDialog`: four Windows save dialogs of the test's own
  (old and new style, Cancel and Save): the file name box was found and filled by
  `WM_SETTEXT` (control id 1148 old style, 1001 new style) and Save pressed by `BM_CLICK`
  (id 1); the dialogs returned the name set. No keys or clicks were sent. This is the path
  `steps-F28.ps1` now takes before falling back to typing.
- `ck-sitting.selftest.ps1` (the frame): 0 failed after the save-rule change.
- The batch: 71 plugins built, each validated and round-tripped (above);
  `ck-batch.ps1 -DryRun` against the real MO2 (read only) lists the three deploy steps per
  plugin; each `kit deploy --dry-run` of the 13 group and twin plugins was refused only for
  MO2's selected profile being `Default` (and `deploy enable --dry-run` for MO2 running and
  the mods not yet deployed); nothing was written under MO2 or the game folder.
- `ck-ingest.ps1` on the existing fixtures: F30 (`ck-20261101-02`) gives CELL and REFR
  `pass`, as `docs/schema/evidence.toml` records; F29 (`ck-20261101-03`) gives CELL `pass`
  and REFR `inconclusive` (the CK's own reference is an ours-missing item; the ledger
  classed it by hand). `-Apply` was tested on copies of the two docs.

## Not verified, and risks

- The CK has never been driven by `ck-plugin.ps1`; every step is built from observed
  behaviour (`ck-20261101-01` to `ck-20261103-01`) but runs together for the first time at
  the bench. Hence the first plugin alone, watched.
- Posted `BM_CLICK` on "Set as Active File": `ck-20261101-02` used `BM_CLICK` and it took;
  whether it was posted or sent is not recorded. If the title never names the plugin, the
  run stops after 60 s of a loaded CK with another title.
- Whether the CK writes `EditorWarnings.txt` during the load or only at exit; both are
  copied.
- What the CK does with KitKraft's minimal `[[new]]` records (EditorID only) for types with
  required members (NPC_, QUST, PACK, planet types): a load warning or a prompt is the
  expected way it shows; a crash costs one group.
- F20's twin holds the cell only; the CK's F20 adds a reference, which the diff shows as
  ours-missing.
- `kit build` output is reproducible from the committed recipes and the same `kit`; the
  manifest names the commit it was built at.
- MO2 rewrites `ModOrganizer.ini` on every launch (recorded, as before); after the sitting
  MO2's selected profile may still be `Kitkraft Dev`.

## Files

- Repo: `docs/bench/ck-sitting.md` (this file); `docs/bench/ck-batch/`: `types/*.toml`
  (58), `groups.txt`, `fixtures/*.toml` (5), `build.ps1`, `ck-ingest.ps1`.
- Scripts (outside the repo, not a git repository; originals of the changed ones kept in
  `scripts\backup-20261115\`): new `ck-plugin-lib.ps1`, `ck-plugin.ps1`, `ck-batch.ps1`,
  `ck-cleanup.ps1`, `ck-plugin.selftest.ps1`; changed `ck-sitting.ps1`
  (`Test-WriteAllowed`: an exact plugin file name) and `steps-F28.ps1` (the file name box by
  `Set-SaveDialogName`).
- Built plugins and manifest: `C:\Modding\kitkraft-observe\ck-2026-11-16\plugins\`.
