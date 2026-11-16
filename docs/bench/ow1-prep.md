# OW-1 bench prep (for the at-desk session of 2026-11-16)

Prepared 2026-11-15 with no game, no MO2 and no GPU, on branch `opus/bench-ow1-20261116`.
The run sheet itself is `docs/game-sessions.md`, "Run sheet OW-1"; this page is what to
check before it, what changed since the failed attempt, and what to do when something goes
wrong. Grades follow AGENTS.md "Reporting": nothing on this page is a game observation.

## 1. Why OW-1 timed out on 2026-11-10 (`game-20261110-01`)

**Cause: verified from the recorded transcript; already fixed on `main` before tonight.**

The evidence is `C:\Modding\kitkraft-observe\2026-11-10-T01\report.json` (SHA-256
`0bd7f1d6...993d0`, identical to the test fixture
`crates/game/tests/fixtures/2026-11-10-T01-report.json`), 38 sends:

| Send | ms after launch | Command | Reply |
| ---- | --------------- | ------- | ----- |
| 1 | 91,485 | `GetSFSEVersion` | echo + `SFSE version: 0.2.21, ...`: ready |
| 2 | 91,589 | `load KitKraftBase` | echo only (as every `load`) |
| 3 | 91,695 | `GetSFSEVersion` | echo only (the game is loading) |
| 4 | 96,799 | `GetSFSEVersion` | echo only |
| 5 to 38 | 101,913 to 270,538 | `GetSFSEVersion` | echo + exactly one `SFSE version` line each |

The controller of that day used the launch wait's send-order count after the base `load`
too: an echo-only poll "owed" a later `SFSE version` line. The game never sent the two
lines that sends 3 and 4 owed (it dropped them), so every later line paid only its own
poll, the debt stayed at 2, and the base-save wait gave up at `load_secs` = 180 s with
every row `not-run`. The game itself was fine: it answered every 5 s for three minutes.
Not the cause: the timeout length, the save's name, path or hash (the backup copy hashes
`fbed2cc2...eb5ccf`), the port, or the plugin.

**Fix (on `main` before this branch, ruling R41, the pacing poll):** after a `load` the
wait is over once `PACING_POLLS` (3) polls in a row had any reply, echo included
(`crates/game/src/controller.rs`, `wait_ready`). The test
`r41_t01s_base_wait_is_ready_at_send_5_and_the_session_goes_on` replays T01's transcript:
ready at send 5 (about 10 s after the `load`), and the session goes on to SO-01. It passes on
this branch.

**What is still open (hypotheses, untested in game):**

- The **launch** wait (before any `load`) still uses the send-order count. If the game
  drops an echo-only launch poll's answer the same way, the launch wait times out at
  `ready_secs` (300 s) with "N earlier poll(s) still owe their answer". In T01 the first
  launch poll (91 s, after the 75 s settle) was answered in full, so this did not happen;
  the settle makes it unlikely, not impossible. The run sheet already says how to grade
  it (rows `not-run`, retry once). Changing it is a grader decision, not made here.
- The game can drop a `load`'s `Save file not found.` the same way; that is the limit no
  grader passes (`docs/game-sessions.md`, "Statuses"). The base save exists and hashes
  right, so it should not arise for the base `load`.

## 2. What this branch changes

| Change | Files | Why |
| ------ | ----- | --- |
| Diagnostics on a timeout: before the game is ended after the launch wait, the base-save wait or a row times out, one finding records the session's processes still running, whether `Starfield.exe` had its main window, and a screenshot `observe\<id>\screenshots\stuck-<n>.png` (or why none was taken) | `crates/game/src/controller.rs` (`Run::diagnose`) | T01 left only the transcript: a hung load, a crash and a console that stopped answering all look like "timeout". Tomorrow's first timeout, if any, is conclusive from `report.json` alone |
| A log the session did not write is recorded as missing: a log whose length and modified time are unchanged since before the launch is not copied, and a finding says so | `crates/game/src/controller.rs` (`copy_logs`, `log_mark`) | T01 copied `StarfieldConsoleHistory.log` last written 2026-11-01 (its last line `qqq`) into `logs\` as if the session had written it |
| Tests against the fakes | `crates/game/tests/session.rs`: `a_log_left_from_an_earlier_run_is_recorded_as_missing`, `a_wait_that_gives_up_records_what_the_box_showed`, `a_row_timeout_records_diagnostics_even_without_a_screenshot` | |
| Preflight script | `scripts/bench/ow1-preflight.ps1` | Everything checkable with no game, in one command (section 3) |
| Doc | `docs/game-sessions.md` steps 7 and "Statuses" | The two behaviours above |

Tests run on this branch (`CARGO_TARGET_DIR=.targets\opus-house`): `cargo test -p
kitkraft-game`: lib 29, adversarial 13, pacing 4 (1 ignored), session 86 (2 ignored),
all pass; `cargo test -p kitkraft-engine --lib handlers::game`: 13 pass; `cargo clippy -p
kitkraft-game --all-targets -- -D warnings` clean; `cargo fmt` clean. The full
`cargo xtask ci` gate was not run here (the integration step runs it).

## 3. Headless preflight

```powershell
cd <worktree>
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\bench\ow1-preflight.ps1 -Kit <kit.exe>
```

It starts nothing and writes nothing. It checks: no `Starfield`, `sfse_loader`,
`CreationKit` or `ModOrganizer` process; MO2's `selected_profile`; `KitKraftBase.sfs` and
its SHA-256; the plugin's SHA-256 and `kitkraft-deploy.json`; whether the mod and plugin are
enabled; the probes file's SHA-256; TCP 45555 unbound and outside Windows' excluded
ranges; free disk; the `kit.exe` hash; the four list files' hashes (the "before" of run
sheet step 1); then `kit game session ... --dry-run --json` (every preflight rule,
including `console-plugin`, `console-port`, `ini-switch`, `profile-settings`,
`plugin-not-deployed`, `unverified-profile`, and the probes file's parse), its timeouts and
its authorized writes. The exit code is the number of FAILs.

Output on 2026-11-15 22:14 (kit debug build of `75a26704`, `kit.exe` `6e6c351c...0cc7e0`):

```text
INFO worktree HEAD: 75a26704
PASS processes: no Starfield, sfse_loader, CreationKit or ModOrganizer
FAIL selected profile: selected_profile=@ByteArray(Default) (the owner selects 'Kitkraft Dev' in MO2, then closes MO2)
PASS base save: ...\Kitkraft Dev\saves\KitKraftBase.sfs fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf
PASS plugin: ...\KitKraft Dev - OldSovietMarksmanRifle\OldSovietMarksmanRifle.esm 9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27
INFO plugin enabled: modlist.txt: True; plugins.txt: True (step 1, deploy enable, makes both true)
INFO probes: ...\crates\game\probes\send-order.toml a8638979acccf83615e02ec81ba60465bf46ea93470390439b354da4f0a1350c
PASS port 45555: nothing bound
PASS port 45555: outside every excluded TCP range
PASS disk: C: 129.2 GB free
INFO list file: modlist.txt bfa10ad4e23fc9ca4b1fef0c4c3e69aad079b7d8a268aacac8da040d337bb204
INFO list file: plugins.txt 286a54cf44506fa62b9596270f03b618a5e337b9d991a579d840a9a622cb9b2d
INFO list file: loadorder.txt 46b28b56b0f5226d913ee655ff5632ec5f86e064f5623a73a62f245945dfcd27
INFO list file: lockedorder.txt 9962fbb07e4c40c4c39b6fe7cedf3f1527cdda5ad0a60d7d53a872b5ae52812d
FAIL dry run [selected-profile]: MO2's selected profile is `Default` (the owner's live game); select `Kitkraft Dev` in MO2 first
INFO timeouts: ready 300s (settle 75s), load 180s, probe 120s, session 1800s
INFO authorized write: C:\Modding\kitkraft-observe\2026-11-16-T01\** (outputs, logs, screenshots, backups)
INFO authorized write: <worktree>\docs\game-observations.md (one appended entry)
INFO authorized write: ...\Kitkraft Dev\{modlist,plugins,loadorder,lockedorder}.txt, the four INIs, saves\KitKraftBase.sfs (rollback only, from its backup)
fails: 2
```

Read: the two FAILs are one cause. MO2's selected profile is `Default` again (on
2026-11-10 the owner had switched it to `Kitkraft Dev` at 15:15). The dry run refused
nothing else: base save present with the run sheet's hash, plugin as built, Console API
enabled with `iPort=45555`, INI switches and `LocalSettings`/`LocalSaves` set.

Two notes for the reader of tomorrow's run:

- The plugin is already enabled in `Kitkraft Dev` (lines left since 2026-11-10 18:40, not
  by a KitKraft session), together with `KitKraftFirstPOI`, `KitKraftGunOverhaul` and the
  two MEL DLC mods. Step 1's `deploy enable` should report nothing to change; record its
  dry-run output anyway.
- The session id follows the controller's date: tonight's dry run named
  `2026-11-16-T01`; a run after midnight on that clock gets the next date.

## 4. Decision tree (symptom, check, action)

Each symptom is read from the dry run, the `kit game session` output or `report.json`
(`findings`, each row's reason, `transcript`). Since this branch, every timeout also has a
`diagnostics at ...` finding and `screenshots\stuck-<n>.png`.

| Symptom | Check | Action |
| ------- | ----- | ------ |
| `[selected-profile]` | `Select-String C:\Modding\MO2-Starfield\ModOrganizer.ini selected_profile` | Owner opens MO2, selects `Kitkraft Dev`, closes MO2. Re-run the preflight |
| `[running]` | `Get-Process Starfield,sfse_loader,CreationKit,ModOrganizer` | Owner closes them (never kill the owner's game). Re-run |
| `[plugin-not-deployed]` | `modlist.txt` has `+KitKraft Dev - OldSovietMarksmanRifle` | Run sheet step 1 (`deploy enable`, dry run first) |
| `[base-save]` | preflight's base-save line | Stop. A backup copy is `C:\Modding\kitkraft-observe\2026-11-10-T01\backup\08-KitKraftBase.sfs` (same hash on 2026-11-15); putting it back into the profile is the owner's write |
| `[console-port]` | `netsh int ipv4 show excludedportrange protocol=tcp`; the mod's `sfse_plugin_console_api.ini` `iPort` | The owner picks a free port in the mod's INI; the profile's `[console] url` must match (a code change) |
| Launch wait: `no main window of Starfield.exe yet` at 300 s | diagnostics: which processes ran; the screenshot | Only `ModOrganizer.exe`: MO2 showed a dialog or the SFSE entry is missing (screenshot shows which). Nothing ran: launch failed (finding `launch failed`). Owner fixes, retry once |
| Launch wait: `answered an empty body` or a connection error at 300 s | diagnostics: `Starfield.exe` running, main window yes | The Console API did not come up: check the port (above) and SFSE's plugin log. Retry once |
| Launch wait: `N earlier poll(s) still owe their answer` | `transcript`: which poll answered only its echo, which replies came after | The launch count (section 1, open item). Grade rows `not-run`, record the polls as send-order evidence (run sheet "Reading the result"), retry once; if it repeats, stop and report to the director |
| Base wait: `the base save did not load: ... Save file not found.` | `saves\KitKraftBase.sfs` in the profile; `settings.ini` `LocalSaves=true` | The game looked elsewhere for saves; stop and report (no row ran) |
| Base wait timeout, reason `had a reply N time(s) in a row of 3` | diagnostics: main window yes or no; screenshot (loading screen, menu, crash dialog) | Window and a loading screen: the load hung; retry once. No `Starfield.exe`: the game crashed on the save; stop and report |
| Base wait timeout, reason `answered an empty body` or a timeout | as above | The console stopped answering after `load`; screenshot decides hang or crash; retry once |
| A row `inconclusive (timeout ...)`, later rows `not-run (timeout at row X)` | diagnostics at that row; the row's `seen` | Game gone: crash on that row's command (record the command). Game up: the console stopped answering; retry the session once |
| `still running after 60s: [...]` | `Get-Process` | End the leftovers by hand before anything else; note it in the reading |
| `Finding: ... plugins.txt changed` | | MO2's known rewrite at launch (`game-20261103-01`); no grade changes |
| `log console-history was not written during the session` | | Expected (the profile has no evidence that the API's commands reach that log); not a fault |
| SO-01 not `pass` | its `seen` | The session tests nothing: `inconclusive`, rerun once in the window |
| SO-05 `pass` and SO-06 `fail`, or a later line before an earlier one | `seen` of both | Contradiction: stop and send to the director at once (run sheet, R17) |

## 5. OW-1 command list with expected durations

From the worktree whose `kit.exe` is the integrated release build (run sheet: `.targets\x\release\kit.exe`;
it did not exist on 2026-11-15 and must be built before the window, never while the game
runs). `$kit` is that path.

| # | Command | Expected | Duration |
| - | ------- | -------- | -------- |
| 0 | Owner: open MO2, select `Kitkraft Dev`, close MO2 | `selected_profile=@ByteArray(Kitkraft Dev)` | 1 min |
| 1 | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\bench\ow1-preflight.ps1 -Kit $kit` | `fails: 0`, `would start session ...-T01` | 10 s |
| 2 | `& $kit deploy enable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev" --dry-run`, then without `--dry-run` | nothing to change (already enabled); record the list-file hashes from step 1 | 10 s |
| 3 | `& $kit game session C:\Modding\MO2-Starfield crates\game\probes\send-order.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger <worktree>\docs\game-observations.md --dry-run` | no `[rule]` line; authorized writes as in section 3 | 5 s |
| 4 | The same without `--dry-run`. Leave the screen alone | Main window, then the 75 s settle; first poll about 91 s after launch (T01); base load wait about 10 s (3 polls); eight rows with five 10 s pauses and getter retries (2 s, 4 s) about 2 to 3 min; exit up to 60 s | about 5 min (bounds: 300 s launch, 180 s load, 120 s a row, 1800 s session) |
| 5 | `Get-Process Starfield,sfse_loader,ModOrganizer -ErrorAction SilentlyContinue` | nothing | 5 s |
| 6 | `& $kit deploy disable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev" --dry-run`, then without (only if the owner wants the rename off the dev profile) | the three list files hash as before step 2, but for the known `plugins.txt` line | 10 s |
| 7 | Read `C:\Modding\kitkraft-observe\<id>\report.json`: rows' `seen`, `findings` (diagnostics, logs), `transcript`; write the reading under the ledger entry (run sheet "Reading the result") | | 15 min |

A second attempt in the same window (run sheet: two attempts) adds about 6 minutes.

## 6. Amendment 2 (input injection and focus): what still blocks it

No preflight rule refuses a session for using input or focus; nothing in the session code
can send input either. These must change in a later ticket (none removed here):

1. `crates/game/src/probes.rs`: `Step` is a console command only, and every probe struct
   is `deny_unknown_fields`, so a `probes.toml` step for a key, click or controller input
   fails to parse. Needs a step kind and a `FORMAT` bump.
2. `crates/game/src/host.rs`: the `Host` trait has no focus or input method (only
   processes, launch, terminate, screenshot, main window, excluded ports), and ADR 0010's
   "Dependency list" names only `tasklist`, `taskkill` and PowerShell capture as process
   tools; an input path is listed there first (a new crate or driver is the owner's,
   Amendment 2).
3. `crates/game/src/controller.rs` and `replay.rs`: the transcript records console sends
   only; Amendment 2 requires input sent to be in the report (what, when), and a replay
   would diverge on it. The module doc's "talks to the game only over" list also needs it.
4. `AGENTS.md` ("Agent-driven sessions": talks to the game "only over the Console API
   plugin's loopback HTTP endpoint, the files ..., and screenshots") and
   `docs/game-sessions.md`'s opening still state the console-only rule.
5. `crates/engine/src/handlers/testplan/probes.rs` (line 481) writes manual rows whose
   reason is "clicking needs input injection (deferred, ADR 0010 Amendment 1 (b))".
6. `docs/foothold/gap-matrix.md`: `OnActivate` is `not-run` citing the same deferral.

Not blocking: `REFUSED_COMMANDS` (console commands only), the `running` rule, and the
`ini-switch` rule's `bAlwaysActive=1` (it keeps the game running unfocused; focus does not
conflict with it). Amendment 2 also says the first work is research only, so OW-1 sends
no input.
