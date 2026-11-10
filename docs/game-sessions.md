# Game sessions (`kit game session`, ADR 0010)

The session controller lets an agent run a plugin's in-game checks on the MO2 profile
`KitKraft Dev` (folder `Kitkraft Dev`) with no owner step. It launches Starfield through
MO2 and drives it over the Console API plugin's loopback HTTP endpoint. It checks the
probes `kit testplan` wrote, records what it saw in `docs/game-observations.md`, and then
ends the game. The rules are ADR 0010 and its Amendment 1. The code lives in `crates/game`
(L3) and in the engine commands `game.session`, `game.status`, `game.probe` and
`game.stop`.

**State (2026-11-01): built and tested against fakes; the Starfield profile has its
launch, console and save evidence.** The first agent-run session (`game-20261101-T1`,
M4.T1 rows a, b, d and h, scripted outside the controller) observed the MO2 command line,
V0 over HTTP on port 45555, `load` from the main menu and the Papyrus log path, so
`unverified-profile` no longer refuses Starfield. The same session showed that an open
port is not readiness (row b'), and the controller now waits for the game's main window
and a settle period before its first command (step 3 below). No session has yet been run
by the controller itself.

## Commands

| Command | Writes | What it does |
| ------- | ------ | ------------ |
| `kit game session --mo2 <instance> --probes <probes.toml> --base-save-sha256 <hex> [--profile "KitKraft Dev"] [--game starfield]` | yes | Preflight, then one whole session, synchronously (the one-shot CLI) |
| `... --start` | yes | The same session on a thread the engine's session owns; returns its id at once (`2026-10-16-T01`) |
| `... --dry-run` | no | The plan (command line, backups, probes, authorized writes, timeouts) and every refusal; writes nothing |
| `kit game status <id>` | no | Phase, the row running, rows so far, re-runs, screenshots (paths under `observe\<id>\`), events; the report once finished |
| `kit game probe <id> --row T-03 [--screenshot]` | yes | Re-runs one row of a session started with `--start`, once its probes are done |
| `kit game stop <id> [--dry-run]` | yes | Ends the session: game ended, logs copied, profile verified, ledger entry; `--dry-run` says what it would do |

Each command is one `CommandSpec`, so the CLI, MCP and the workbench (M6.G4) share one
implementation. `game.session` also reports progress as it runs: a `preflight` event
before and after the facts are gathered (in every form, and a refused session still
sends the first), and, in the synchronous form, a `phase` event each time the controller
changes phase and a `probe` event each time a probe row finishes. A front end that
stays open follows a session through `game.status`, and it needs no second engine
worker. A started session belongs to the engine that started it. When that engine goes
away, the session is stopped properly: the game is ended, the profile verified and
the ledger entry written. With the one-shot `kit` CLI that happens as soon as the
command returns, so use the synchronous form there and `--start` from MCP or the window.

## Preflight (every condition refuses, naming its rule)

| Rule | Refused when |
| ---- | ------------ |
| `live-profile` | the profile named is `Default` |
| `profile-missing` | the profile folder is not in the named MO2 instance |
| `selected-profile` | MO2's `selected_profile` is absent, `Default`, or another profile |
| `running` | `Starfield.exe`, `sfse_loader.exe`, `CreationKit.exe` or `ModOrganizer.exe` runs |
| `profile-settings` | the profile's `settings.ini` lacks `LocalSettings=true` or `LocalSaves=true` |
| `ini-switch` | a switch of the game profile is missing from the profile's INI (`[General] bAlwaysActive=1`, `[Papyrus] bEnableLogging=1`, `bEnableTrace=1`, `bLoadDebugInformation=1`) |
| `console-plugin` | no enabled mod of the profile is the Console API |
| `console-port` | the console URL's port lies in a TCP range Windows excludes from binding (`netsh int ipv4 show excludedportrange protocol=tcp`), or the enabled Console API mod declares another port in its own config (`[console.port_config]`: `SFSE\plugins\sfse_plugin_console_api.ini`, `[WebConsole] iPort`) |
| `base-save` | `saves\<base>.sfs` is missing, its SHA-256 was not given, or it differs |
| `plugin-not-deployed` | the plugin of `probes.toml` is not a `kit deploy --dev` mod (`KitKraft Dev - <stem>` with `kitkraft-deploy.json`) enabled in the profile |
| `unverified-profile` | the game profile has a fact without evidence (below) |

Two of these show what the dev box needs from the owner (M4.O7). First, on 2026-10-16
`Kitkraft Dev\settings.ini` says `LocalSettings=false`. With that setting the game reads
the INIs under `Documents\My Games\Starfield`, not the profile's. Profile-specific game INI
files must be switched on in MO2 before the profile's switches take effect. Second, the
profile's `StarfieldCustom.ini` has none of the switches yet. The controller only reads
INIs and never writes them.

The console endpoint is `http://127.0.0.1:45555/console`, not the plugin's default
55555. On the dev box Windows excludes TCP 55477-55576 (Hyper-V/WSL reservations), the
plugin could not bind 55555, and the owner set `iPort=45555` in the mod's
`sfse_plugin_console_api.ini` (`plan/STATUS.md` "Owner next actions" row 10, 2026-11-01).
`console-port` catches the same trap on any box. The excluded ranges come from `netsh`
through the host; when they cannot be read (not Windows, or `netsh` failed), or the mod has
no such config file or key, nothing is refused, and the result's `limitations` and the
ledger entry's "Console port" line say what was not checked.

## A session

1. Back up `modlist.txt`, `plugins.txt`, `loadorder.txt`, `lockedorder.txt`, the
   profile's `*.ini` and the base save into `observe\<id>\backup\`, with their SHA-256.
2. Start `ModOrganizer.exe -p "<profile>" "moshortcut://:SFSE"` (observed in
   `game-20261101-T1`, row a: the game window came up after 14 s).
3. Wait for readiness, sending nothing until the game can take a command. The plugin's
   port accepts connections about 5 s after launch, long before that, and in
   `game-20261101-T1` (row b') a `GetSFSEVersion` sent 8 s after launch answered an empty
   body and the game ended. So the controller:
   - sends no command until `Starfield.exe` has a main window (the host's window list),
     and then none for `[timeouts] settle_secs` (Starfield: 75 s, the wait that worked);
   - then polls the readiness command (`GetSFSEVersion`) every `ready_poll_secs` (5 s).
     An empty or blank body is not ready, and neither is one holding only the command's
     echo (the shape of every late reply); with `ready_expect` empty any other answer is,
     otherwise the answer must contain it. Starfield's `ready_expect` is still empty; it
     becomes `SFSE version`, the text `game-20261101-T1` row b observed after the echo
     (`SFSE version: 0.2.21, release idx 26, runtime 01100F40`), together with the engine's
     test fakes answering the same (G 3b review M1);
   - gives up after `ready_secs` from launch (300 s), which covers the window, the settle
     and the polls (the first answer came at 91 s). A profile whose `settle_secs` is not
     below `ready_secs` is an error. The row reasons then name what was still missing (no
     window yet, settling, or the last answer).

   Then it sends `load <base save>` and waits for a non-empty answer again (`load_secs`).
   A `load` step in a row is followed by the same wait. The poll's replies are replies like
   any other (G ticket 3b): a late line of an earlier request may arrive in one, so after a
   row's `load` each reply is routed as described in "Statuses" and noted in the row's
   `seen` as `(readiness poll) GetSFSEVersion -> ...`.
4. Run the probes in order. A row has its own deadline (`probe_secs`), and the session
   has a wall-clock budget (`session_secs`). After a step with `wait_secs` the controller
   pauses that long (within the row's deadline; `game.stop` cuts it short) before the next
   step, and the row's `seen` notes `(waited Ns)`. The controller sends no `save` or `load` to a
   name it does not own, and it never sends `bat`, `saveini`, `qqq` or `quitgame`.
5. With `--start`, stay open for `game.probe` until `game.stop` or the budget runs out.
6. End `Starfield.exe`, `sfse_loader.exe`, MO2's helpers and MO2 itself. Preflight
   guaranteed none of them ran before the session, so the session started them all.
7. Copy the logs (`Papyrus.0.log`, `StarfieldConsoleHistory.log`) into `observe\<id>\logs\`.
   A log that was not written is recorded as missing.
8. Hash the backed-up files again. A changed file that existed before is restored byte
   for byte from its backup. A file that appeared is reported and left in place, because
   rollback only restores. Any change is a finding (file it under `plan/STATUS.md`
   "Requests"), and the session is then `inconclusive`. The plugin under test is rolled
   back only with `kit deploy rollback`.
9. Append one entry to `docs/game-observations.md` and write `observe\<id>\report.json`.

### Statuses

Each row is exactly one of `not-run`, `pass`, `fail` or `inconclusive`, and every status
except `pass` carries its reason.

- A console failure line in any step's answer makes the row `fail`, whatever its later
  matchers say: `Item '...' not found for parameter ...`, `Script command "..." not
  found.`, `Save file not found.` (the shapes of `game-20261101-T1` rows i to l). The
  reason names the step and the line, as in ``fail (step 3 `load KitKraftSession-...`: the
  console answered `Save file not found.`)``. The command's own echo is never read as its
  failure. The other steps still run, for the evidence. If a timeout follows, the row
  stays `fail`, its reason adds the timeout, and the session ends as for any timeout.
  Why: in `2026-11-01-T03`, row T-05, the `load` found no save (it was sent 120 ms after
  the `save`, before the file was written), and the final `player.getincell` passed only
  because the player never left the cell.
- A base save the game does not find (`Save file not found.` after `load <base save>`, in
  the `load`'s own answer or late, in the readiness poll after it) leaves every row
  `not-run (the base save did not load: ...)`.
- After a row's `load`, the readiness poll is a request of that step that names nothing,
  and its replies are routed by the send-order rule (`crates/game/src/correlate.rs`): a late
  getter line reaches its getter, a late failure line re-opens the rows it may belong to
  (`inconclusive (late failure)`), and a failure line left with the poll is the `load`'s,
  late, so the row is `inconclusive (late failure: ... in the readiness poll ...)`, never
  `pass`. Before G ticket 3b these replies were dropped, and a late `Save file not found.`
  there let the row pass on getters that saw the old state.
- `inconclusive` means the evidence is missing or ambiguous. Examples: no output to match,
  no number in the output, a Papyrus log that was not written, a screenshot the row asks
  for that was not taken, or a timeout. Missing evidence is never `pass`.
- A probe that times out is `inconclusive (timeout ...)`. Every later row is
  `not-run (timeout at row X)`, and the game is ended.
- `game.stop` makes the remaining rows `not-run (stopped by game.stop before row X)`.
- A `manual` row is `not-run (manual: <reason>)`. If it asked for a screenshot, it is
  `inconclusive` until the owner looks (Amendment 1 condition 8).
- The session's status is the worst of its rows (`fail`, then `inconclusive`, then
  `not-run`). A changed profile file makes the whole session `inconclusive`.

### Writes (Amendment 1 condition 3)

The controller writes only these paths, and `kitkraft_game::authorized::Authorized`
refuses any other: `observe\<yyyy-mm-dd>-T<nn>\**`, one appended entry in
`docs/game-observations.md`, and the backed-up profile files, which it touches only to
restore them. The game itself makes the `KitKraftSession-<yyyymmdd>-<nn>` saves when the
controller asks it to. Tests run whole sessions on a fixture MO2 instance and assert every
written path against this list. They also assert that the instance is byte-identical
afterwards (`crates/game/tests/session.rs`, `crates/engine/src/handlers/game/tests.rs`).

## `probes.toml` (M4.T2)

`kit testplan` writes `probes.toml` beside `checklist.md`, one probe per checklist step.
The step's number is the probe's row (`T-<nn>`).

```toml
format = 1
plugin = "KitKraftFirstPOI.esm"
sha256 = "..."
profile = "KitKraft Dev"
runtime_id_basis = "inferred:runtime-slots-by-scale"

[[probe]]
row = "T-02"
title = "player.moveto 06001000: stand at REFR `KitKraftFirstPOICrate`"
forms = ["KitKraftFirstPOI.esm:001000", "KitKraftFirstPOICrate", "06001000"]

[[probe.step]]
command = "player.moveto 06001000"
wait_secs = 10

[[probe.step]]
command = "prid 06001000"

[[probe.step]]
command = "getdistance player"
matcher = "number"
expected = "0"
tolerance = 500.0
why = "player.moveto puts the player beside the reference"

[[probe]]
row = "T-01"
title = "go to cell ... (exterior)"
matcher = "manual"
reason = "an exterior cell has no `coc` target here; ..."
```

Matchers:

| Matcher | Passes when |
| ------- | ----------- |
| `contains` | The output contains `expected` (ASCII case ignored) |
| `regex` | The output matches `expected` |
| `number` | The number right after the output's last `>>` is within `tolerance` of `expected`; an output with no `>>` has no number (`inconclusive`) |
| `papyrus-log` | The Papyrus log written during the session contains `expected` |

A step without a matcher is setup. `{save}` stands for the session's own save name and
`{base_save}` for the base save's name. A step's optional `wait_secs` (at most 300) is a
pause after its answer, before the next step; it counts against the row's `probe_secs`, so
a wait that would run past the deadline is cut there and the next step times out. Every
step's answer is also checked for a console failure line, whether or not the step has a
matcher (see "Statuses").

What the console does, observed on Starfield 1.16.244 over the Console API
(`game-20261101-T1`):

- An answer is the command's echo, then `Command: <command>`, then the result lines; a
  getter's result is `GetPos: X >> -17.00`, `GetInCell >> 1.00`, `GetDistance >> 9.48`.
  The `number` matcher reads the number after `>>`, so a digit of the echo
  (`player.moveto 06001000`) is never taken for an answer, and `kit testplan`'s regexes
  look for `>> 1` at the end of a line.
- The console does not resolve a reference's EditorID: `prid KitKraftFirstPOICrate`
  answers "Item 'KitKraftFirstPOICrate' not found for parameter ObjectReferenceID.",
  `help KitKraftFirstPOICrate 4` lists nothing, so `player.moveto <EditorID>` cannot
  work. Its runtime FormID does: `prid 06001000`, then `getpos x`, `getpos y`, `getpos z`
  gave -17.00, -4.00, 4.63. A cell's EditorID in `player.getincell` gets no answer; its
  FormID does (`player.getincell 0021966F` and `21966F` gave `GetInCell >> 1.00`). So
  `kit testplan` names references and cells by their runtime ID (8 hex digits) in `prid`,
  `player.moveto`, `getisid` and `player.getincell`, and keeps the EditorIDs in titles
  and `forms`. `coc` still takes the cell's EditorID (grade D).
- `isdisabled` is not a Starfield console command ("Script command \"isdisabled\" not
  found."). `kit testplan` no longer probes the initially-disabled flag; no replacement
  was observed.
- `player.moveto`, `coc` and `load` answer at once (the echo only) and the game then
  moves; a command sent meanwhile answers with only its echo. `kit testplan` gives those
  steps `wait_secs = 10` (a `load` from the main menu answered a getter within 10 s).
- `save` also answers before its file exists: a `load` of it sent about 120 ms later
  answered "Save file not found." and the file appeared moments after
  (`2026-11-01-T03`, row T-05). `kit testplan` gives `save {save}` `wait_secs = 10`.

The persistence row (`persistence 1-3`) proves that the reload happened by checking the
player's cell, which differs before and after the load. The row runs `coc
CityNewAtlantisLodgeInt` and checks `player.getincell <Lodge ID>`. Then it runs `save
{save}` and goes to another cell: the plugin's own interior cell, or else
`CityNewAtlantisApartmentTEMPLATE`, which `2026-11-01-T03` row T-01 observed with `coc` and
`player.getincell 0021966F`. There it checks `player.getincell <other ID>`. Then it runs
`load {save}` and checks `player.getincell <Lodge ID>` again. Every one of these commands
was observed working, and a `1` after the load can only come from the save. If the load
order has no runtime ID for the Lodge or for any other cell, the row keeps the weaker
check after the load (`player.getincell <Lodge ID>`, or `player.getpos x` without a Lodge
ID), and its `why` says that it does not prove the reload. With its four 10 s pauses and
the load, the row needs more than the old 60 s deadline, so the Starfield profile's
`probe_secs` is now 120.

Everything else the probes send (`getisid`, `player.getitemcount`, `player.getequipped`,
`getstage`, `save` and `load` from the world) is grade D until a session observes it.

A reference placed from a pack-in (`PKIN`) gets no `getisid`: its base row is `manual`
(KK-250, `probes::PACK_IN_BASE`). `getisid` on the PKIN's runtime ID answered `GetIsID >> 0.00`
in game-20261103-03 and game-20261103-08 while the reference stood where the plugin puts it;
whether the game reports a PKIN as such a reference's base is open.

## Game profiles (M4.T9)

`crates/game/profiles/<game>.toml` lists everything the controller knows about one game,
and nothing in it is read from a game binary.

| Section | Keys |
| ------- | ---- |
| `[game]` | `id`, `name`, `status` (`verified`, `partial`, `unverified`), `note` |
| `[executables]` | `game`, `script_extender`, `editor`, `mod_manager`, `helpers` (file names) |
| `[launch]` | `mo2_executable` (the MO2 executable title), `args` (`{profile}`, `{executable}`), `evidence`, `pending` |
| `[console]` | `url` (loopback `http://` only), `ready_command`, `ready_expect`, `mod_names`, `candidates`, `evidence`, `pending` |
| `[console.port_config]` | optional: `file` (relative to the console plugin's mod folder, `/`-separated), `section`, `key` (where the plugin's own config names its port) |
| `[saves]` | `dir` (in the profile), `extension`, `load_command`, `save_command` (`{save}`), `base_default`, `session_prefix`, `evidence`, `pending` |
| `[mo2]` | `local_settings`, `local_saves` (required `settings.ini` values) |
| `[[ini]]` | `file`, `section`, `key`, `value` |
| `[[log]]` | `name`, `path` (`{documents}`), `evidence`, `pending` |
| `[timeouts]` | `session_secs`, `probe_secs`, `ready_secs`, `settle_secs` (after the main window, before the first command; below `ready_secs`), `ready_poll_secs` (0: the controller's poll), `load_secs`, `exit_secs` |

`evidence` names the ledger entry that observed a fact. While `launch`, `console` or
`saves` has none, the controller refuses a live session and names each missing fact with
its ticket. Logs without evidence do not block a session; a log that is missing makes its
rows `inconclusive`.

| Profile | Status | Missing |
| ------- | ------ | ------- |
| `starfield` | partial | nothing that blocks: launch, console and saves cite `game-20261101-T1` rows a, b, d, the Papyrus log row h; the console-history log has no evidence yet, and `load` from the world is unobserved |
| `skyrimse` | unverified (stub) | everything; console candidates: a ConsoleUtil-class plugin, AutoTest, a skytest-style probe plugin |
| `fallout4` | unverified (stub) | as `skyrimse` |

## Template rows and probes

Every row of the ledger's templates O2 and O4 is either a probe the controller can run
(row ids as `kit testplan` numbers them for the named plugin) or a manual row with a
reason. A test (`every_template_row_maps_to_a_probe_or_a_manual_row`) keeps this table
complete.

| Row | Kind | How |
| --- | ---- | --- |
| O2-01 | probe | rename plugin: `help "<FULL>" 4` contains the weapon's runtime ID (the inventory screen itself is a screenshot for the owner) |
| O2-02 | manual | installing a package through MO2 is the owner's; the controller installs nothing |
| O2-03 | manual | as O2-02 |
| O2-04 | manual | the controller never sends `bat`; its rows send the same commands one by one |
| O2-05 | probe | POI `T-01`: `coc <cell EditorID>`, `player.getincell <cell runtime ID>` |
| O2-06 | probe | POI `T-02` (`player.moveto <runtime ID>`, `getdistance player`) and `T-03` (`prid <runtime ID>`, `getisid <base runtime ID>`) |
| O2-07 | probe | POI `T-04`: `prid <runtime ID>`, `getpos x/y/z` against `DATA`; screenshot (no `isdisabled`: not a Starfield command, `game-20261101-T1`) |
| O2-08 | probe | POI `T-05` (leave, `save {save}`, go to another cell, `load {save}`, back in the leave cell) and `T-06` (return, `getpos` again) |
| O2-09 | manual | the long absence (168 game hours) has no chosen console route (FTH-EXP-07) |
| O2-10 | manual | `placeatme` makes a reference whose ID is known only at run time (PS-09) |
| O2-11 | probe | POI `T-03`: `prid <runtime ID>` with `getisid` on the base; `help KitKraftFirstPOICrate 4` itself lists nothing (`game-20261101-T1`: the console does not resolve a reference's EditorID), so the row's expectation cannot hold as written |
| O2-12 | manual | the map and location UI |
| O2-13 | manual | `n/a` for packages without an archive |
| O4-01 | manual | the "Take" or "Steal" prompt is UI; the ownership plugin's probes cover persistence only |
| O4-02 | manual | crime and bounty are UI |
| O4-03 | probe | actors `T-02` (`player.moveto <runtime ID>`) and `T-03` (`getisid <base runtime ID>`); the sandboxing walk is a screenshot for the owner |
| O4-04 | probe | actors persistence rows (`save {save}`, `load {save}`, `getpos` within tolerance) |
| O4-05 | manual | the outpost build menu is UI |
| O4-06 | manual | the fabricator is UI |
| O4-07 | probe | scripts recipe `T-05` (M4.T6): a `papyrus-log` step expecting the `Text` value the reference's `VMAD` sets, which `HelloOnLoad`'s `Debug.Trace(Text)` writes on `OnLoad` (`testplan` reads the `.pex` next to the plugin); the notifications on screen stay a screenshot for the owner |
| O4-08 | manual | a procedural landing (M4.E10) |
| O4-10 | manual | needs the M4.E9 script recipe (PS-06) |
| O4-11 | manual | needs M4.E9 (PS-09) |
| O4-12 | manual | needs M4.E9 (PS-10) |
| O4-13 | manual | `disable` is not in the probes `kit testplan` writes, and `isdisabled` is not a Starfield console command (`game-20261101-T1`): no console check of the disabled state is observed yet |
| O4-14 | manual | needs CK fixture F25 |
| O4-15 | manual | needs F25 |
| O4-16 | manual | needs M4.E9 (PS-19) |
| O4-17 | manual | needs M4.E7 and M4.E9 |
| O4-18 | manual | moving objects and the long absence (PS-23, PS-26) |
| O4-19 | manual | as O4-18 in a never-resets location |
| O4-20 | manual | a procedural site landed on twice (PS-32) |
| O4-21 | manual | needs M4.E9 (PS-39) |

## Testing against the fakes

The workbench and the MCP server test their game buttons and tools against the same
fakes the engine's own tests use: `kitkraft_engine::testing::GameFakes` stands up a
fixture MO2 instance, a fake `/console` endpoint and a fake process host under
`std::env::temp_dir()` (one folder per `tag` and process id), `install` puts the fake
host into an engine's session, `session_args` builds the `game.session` arguments with
extras such as `start` or `dry_run` merged in, and the fakes remove their folder when
dropped.

## Run sheet OW-1: the send-order session (P2)

Written at the desk on 2026-11-10 (sprint 2026-11-09, G1); **nothing in it has been run.**
It runs first in the first game window the owner names, before any other session (P2).
It tests the owner's ruling of 2026-11-07 (midday), item 2: the game console answers in
send order (`plan/STATUS.md`, "Owner rulings, 2026-11-07 (midday)"). The grader builds on
that ruling for the rows where a console failure line arrives while an older getter is
still waiting (`crates/game/src/correlate.rs`; FR2 to FR4 in `crates/game/tests/session.rs`).
If the session contradicts the ruling, the ruling reverts and the grader's send-order rule
goes with it (the integrator runs `git revert -m 1 dc755b49` before the next merge), and
later grades are re-run.

### Inputs

| Input | Value | Check |
| ----- | ----- | ----- |
| Probes | `crates/game/probes/send-order.toml` (hand-written; eight rows, `SO-01` to `SO-08`) | Record its SHA-256. The tests `the_send_order_probes_file_parses_and_names_the_rename_plugin`, `the_send_order_probes_grade_as_the_ruling_predicts` and `a_late_failure_in_the_readiness_poll_after_a_load_is_not_a_pass` pass on the commit `kit` is built from |
| Plugin | `OldSovietMarksmanRifle.esm`, already deployed as the mod `KitKraft Dev - OldSovietMarksmanRifle` (disabled in `Kitkraft Dev` on 2026-11-10) | The mod's file hashes `9c0b28a6e9c4cdf4c7878cf7dd745a022cf6e0ae943d2ad3da6c8f044078ea27` (`docs/smoke-rename.md`; checked 2026-11-10). If it differs, rebuild with `kit build docs\smoke-rename.toml <out> --overwrite` and redeploy with `kit deploy <out> C:\Modding\MO2-Starfield --dev --profile "Kitkraft Dev" --dry-run`, then the same without `--dry-run` |
| Base save | `KitKraftBase` in `Kitkraft Dev\saves` | SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf` (`game-20261101-T1`, `-04`) |
| `kit` | the release build of `ws/g` (`.targets\x\release\kit.exe`), its SHA-256 in the sprint's G report | built before the window, never while the game runs |

### Gates, all before step 1

- The director's message says "window open: OW-1, until hh:mm", with at least 30 minutes
  left. No window, no session (R5, R20).
- The owner has closed MO2 (R22). No `Starfield.exe`, `sfse_loader.exe`,
  `CreationKit.exe` or `ModOrganizer.exe` runs (`Get-Process`); the preflight refuses
  otherwise (`running`).
- At most two cargo builds on the box, `CARGO_BUILD_JOBS=4`, no cold build.
- Profile `Kitkraft Dev` (the folder's spelling), never `Default`.

### Commands, in this order

1. `kit deploy enable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev" --dry-run`,
   then the same without `--dry-run`. It writes only that mod's lines in the three list
   files, after backing them up (ADR 0009 Amendment 2). Record the three files' SHA-256
   before and after.
2. `kit game session C:\Modding\MO2-Starfield crates\game\probes\send-order.toml --base-save KitKraftBase --base-save-sha256 fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf --profile "Kitkraft Dev" --observe C:\Modding\kitkraft-observe --ledger <worktree>\docs\game-observations.md --dry-run`.
   Read every refusal; on any, stop and record its rule. The authorized writes must be only
   `C:\Modding\kitkraft-observe\<date>-T<nn>\`, the `KitKraftSession-*` save and one
   ledger entry.
3. The same command without `--dry-run`, synchronously. Leave the screen alone. About
   five minutes: launch and settle (about 90 s), the base save, eight rows with five 10 s
   pauses, and the getters' retries.
4. After: no game process left; `kit deploy disable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev" --dry-run`,
   then the same without `--dry-run` (the rename owns no form, so it may leave the profile); the three list files hash as
   before step 1. A `Finding:` line about `plugins.txt` is the known MO2 rewrite at launch
   (`game-20261103-01`) and changes no grade.
5. Read `observe\<id>\report.json`: each row's `seen` (every send, its reply clipped to 120
   characters, each retry, each `(late) ... (in the reply to ...)` and each re-queued
   line) and the copied `logs\StarfieldConsoleHistory.log` if the game wrote one.

### How the rows make late answers

`coc` answers at once with its echo and the game then moves; a command sent meanwhile
answers with only its echo, and its result comes in the reply to a later request
(`game-20261101-T1`, `2026-11-03-T05`, `-T10`). Every `coc` but SO-01's is sent with no
wait, so the commands after it answer late on purpose. A getter that answers only its echo
is sent again twice after a pause (R20), and each send gets its own answer. A step with
`wait_secs = 10` ends the late stretch.

### What each row predicts, and what a contradiction looks like

Send order means: the result lines of earlier requests come before those of later ones,
within one reply and across replies. A reply holds its own echo, then any late lines in the
order their requests were sent, then its own result.

| Row | Shape | Predicted grade | In send order the replies show | A contradiction |
| --- | ----- | --------------- | ------------------------------ | --------------- |
| SO-01 | control: a settled `coc` to the Lodge, then `player.getincell 0006BB29` | pass | `GetInCell >> 1.00` in its own reply | none possible; anything but `pass` means the session tests nothing (below) |
| SO-02 | order: `player.getpos x`, `y`, `z` behind an unsettled `coc` | pass (each line goes to its getter by the axis) | every `GetPos: X` line before any `GetPos: Y` line, every `Y` before any `Z`, wherever they arrive | a `GetPos: Y` (or `Z`) line before the last `GetPos: X` line of an earlier send has arrived, for example `player.getpos y -> ... GetPos: Y >> ... GetPos: X >> ...` |
| SO-03 | FR4: `prid RussianHuntingRifle` behind an unsettled `coc`, then `player.getpos z` | not pass (its `prid` fails, late) | the `Item 'RussianHuntingRifle' not found ...` line before SO-03's `GetPos: Z` lines | a `GetPos: Z` line of SO-03 before any failure line, while SO-03's `prid` reply was only its echo |
| SO-04 | FR4: the same `prid` again, then `player.getpos x` (a 10 s settle after) | not pass (its own `prid` fails too) | SO-03's late lines first, then SO-04's own failure line and `GetPos: X` | SO-04's `GetPos: X` before a late line of SO-03 in the same reply, or SO-03's late lines arriving after SO-04's `GetPos: X` |
| SO-05 | FR3: `KitKraftSendOrderRef.enable` (no such reference) behind an unsettled `coc`, then `player.getpos x` | not pass (its `enable` fails, late) | the failure line before SO-05's `GetPos: X` lines | SO-05's `GetPos: X` before the failure line while the `enable` reply was only its echo |
| SO-06 | FR3: `player.getincell 0021966F` (a 10 s settle after) | not fail (its own getter answers) | SO-05's late lines, then `GetInCell >> 1.00` | `GetInCell >> 1.00` before a late line of SO-05 in the same reply |
| SO-07 | FR2 on time: `player.getincell 0021966F`, `load {save}` (the session save does not exist yet), the getter again | fail on the load's own line `Save file not found.` | `Save file not found.` in the `load`'s own reply | the failure line in a reply before the first getter's `GetInCell` line |
| SO-08 | FR2 late: `player.getpos y` and `load {save}` behind an unsettled `coc`, then `player.getincell 0006BB29` (a 10 s settle after) | not pass (its `load` fails, late); the grade names `Save file not found.` wherever the line arrives | SO-08's `GetPos: Y` lines before `Save file not found.`, in the `load`'s reply, in the readiness poll (`(readiness poll) GetSFSEVersion -> ...` in `seen`) or in the last getter's reply | `Save file not found.` before a `GetPos: Y` line still owed, or after `GetInCell >> 1.00` in the same reply |

The grade column is what the controller writes under the ruling. The test
`the_send_order_probes_grade_as_the_ruling_predicts` checks it against a console model that
answers in send order, with zero to six echo-only sends after each unsettled move. **The
grades show a contradiction only in one shape**: when the late stretch after SO-05's `coc`
reaches SO-06 (in the model, four or more echo-only sends; fewer, and the ruling can be
judged only from `seen`, clipped at 120 characters), a console out of send order makes SO-05 `pass` (its `enable` failed) and
SO-06 `fail` on SO-05's line, both against their predictions
(`a_console_out_of_send_order_breaks_the_fr3_pair_only_when_lateness_crosses_a_row`, a
model that answers its own lines first and the late ones newest first). Otherwise the
correlator routes a getter's line by its function and axis, and a failure line by the item
it quotes, whatever the order, so the grades match; the order is read off the replies in
`seen`. Two identical lines (SO-03's and SO-04's failure lines, or the answers to one
getter's three sends) cannot be told apart and are evidence neither way.

### Reading the result (the writer's reading under the ledger entry)

- **Held**: at least one line arrived late (a `(late)` note or a re-queued line in some
  row's `seen`), and every reply holding lines of two or more requests has them in send
  order. The ruling stands; record "send order held" and the rows that showed it.
- **Contradicted**: one reply, or two replies read in turn, has a later request's line
  before an earlier, still-waiting request's line; or SO-05 is graded `pass` and SO-06
  `fail` (above). **Stop and send it to the director at
  once** (R17), quoting the reply text, the rows and the send order. Grade nothing else in
  the entry until the director answers.
- **Inconclusive**: no line arrived late (every getter answered in its own reply, or the
  clipped replies cut off before the order shows), or SO-01 did not pass. Nothing was
  ordered and the ruling is untested. Run the session once more in the same window if
  time allows (two attempts, then `inconclusive` with a request to the session lane),
  never `pass`.
- A `Save file not found.` in a `(readiness poll)` reply of SO-08's `seen` is a late
  answer, routed like any other since G ticket 3b, not a contradiction; it is one only if
  it stands before a line owed to an earlier request, as in the table.

Grades follow AGENTS.md "Reporting": a row with no observation is `not-run`; ambiguous
evidence is `inconclusive`, never `pass`.

## Run sheet OW-2: M6 end to end from the window, and the M7 exit plugin (G2)

Written at the desk on 2026-11-10 (sprint 2026-11-09, G2); **nothing in it has been run.**
The owner is at the desk and watches; an agent drives the release `kit-ui` by computer use,
and nothing else (no game window, no MO2, no CK by computer use). It closes M6's "game
loop, end to end" (M6.G4, M4.T4) up to the owner's M6.O4 sign-off, which is offered at the
end, and makes the M7 exit plugin `KC_F31.esp` (fixture F31, R13) that the CK sitting
opens and re-exports later (pipeline C, OW-C launch 2). It runs after OW-1 (G1), never
before it: a contradiction of the send-order ruling changes how the window's grades read.

The six-status column names the cell of AGENTS.md's hand-off block a step fills: `parse`,
`byte_roundtrip`, `ck_open`, `ck_reexport`, `game_runtime`, `save_reload`; "-" for a step
that fills none. `ck_open` and `ck_reexport` stay `not-run` in this window (pipeline C's).

### Gates and inputs

- The director's message says "window open: OW-2, until hh:mm" and the owner is at the
  desk. Computer use is granted for `kit-ui` alone.
- MO2 is closed (R22); no `Starfield.exe`, `sfse_loader.exe`, `CreationKit.exe` or
  `ModOrganizer.exe` runs. At most two cargo builds, `CARGO_BUILD_JOBS=4`, no cold build.
- Release `kit.exe` and `kit-ui.exe` in `.targets\x\release`, their SHA-256 in the sprint's
  G report, built before the window.
- Base save `KitKraftBase`, SHA-256 `fbed2cc2f3fa0a09046c9818e34f48f1d4ac928739984ffbfe5d81f36eeb5ccf`.
- Before the window, from the worktree (no game, no window): `kit build docs\smoke-rename.toml out\OldSovietMarksmanRifle.esm --overwrite`
  (SHA-256 `9c0b28a6...78ea27`) and `kit testplan out\OldSovietMarksmanRifle.esm out\rename-plan --plugins Starfield.esm`
  (record the `probes.toml` SHA-256; `d84e011a...a55dc05` on 2026-11-02).
- `kit deploy enable C:\Modding\MO2-Starfield --mods OldSovietMarksmanRifle --dev --profile "Kitkraft Dev"`
  (`--dry-run` first): the window has no enable button (ADR 0007 Amendment 3 lists
  `deploy`, `deploy.rollback` and the session commands only).

### Part 1: V1 (the rename plugin) from the Cockpit

| Step | Command (what the window sends) | Expected | Six-status cell |
| ---- | ------------------------------- | -------- | --------------- |
| 1.1 | `.targets\x\release\kit.exe ui --dev`, from the worktree | the window opens on the `KitKraft Dev` profile; record the seconds from start to "loaded" (KK-040's start-up timing) | - |
| 1.2 | Cockpit: pick `out\OldSovietMarksmanRifle.esm`; **Review** (`validate`, `roundtrip --verify`, then `diff` against `Starfield.esm` since the plugin is not in the loaded order) | `validate` 0 errors, 0 warnings; roundtrip identical; the diff shows one WEAP, `FULL` changed; the validation block shows `parse` pass, `byte_roundtrip` pass, the other four `not-run` | `parse`, `byte_roundtrip` |
| 1.3 | **Status** (`deploy.status`), then **Preview deploy** (`deploy --dry-run`) | the dev mod `KitKraft Dev - OldSovietMarksmanRifle` exists, enabled after the gate step; the preview lists every path and the plugin's SHA-256 `9c0b28a6...` | - |
| 1.4 | **Deploy...**, then the confirm box's button (one confirmed `deploy`) | one request per click; the panel reads the dev mod back; the button is enabled only while idle (KK-034) | - |
| 1.5 | Game session: probes `out\rename-plan\probes.toml`, base save SHA-256 as above; **Preview session** (`game.session --dry-run`) | the result draws under the button: `would_start`, no `[rule]` refusal, the plan's authorized writes are `observe\<id>\`, the session save and one ledger entry | - |
| 1.6 | **Launch...**, then the confirm button (`game.session` with `start: true`) | the session id returns at once; the window polls `game.status` every second and shows the phase, the current row, rows done, elapsed seconds and the last events (KK-032) | - |
| 1.7 | Wait for the probes to finish (phase idle) | T-01 (`help "Old Soviet Marksman Rifle" 4` contains `0021BBCD`) and T-02 (persistence by the cell) graded with their reasons; T-03 `not-run (manual: ...)` | `game_runtime` (T-01), `save_reload` (T-02) |
| 1.8 | Select T-02's row, then its **Screenshot** button and the confirm (`game.probe` with `screenshot: true`) | the re-run joins the table as the latest; its screenshot shows below the table, scaled to the panel; a missing or undecodable file is `inconclusive` with its reason, never hidden (KK-033) | - (evidence for T-02) |
| 1.9 | T-03's **Record in Ledger** | the Ledger tab opens with a pre-filled `custom` row (`note`); nothing is written until its own confirmed `ledger` write; the owner says what the row records and the agent confirms it | - |
| 1.10 | **Preview stop** (`game.stop --dry-run`), then **Stop...** and the confirm | the `would` lines, then the stop report: the game ended, logs copied, the profile verified, one ledger entry `game-2026111x-nn` started from the window | the entry's six rows |
| 1.11 | Relaunch: **Preview session** and **Launch...** again, then at once **Preview stop**, **Stop...** and the confirm | the same flow a second time in one window (KK-032's relaunch); its entry may be `not-run` rows only | - |

Acceptance of part 1: a ledger entry started from the window with the six statuses filled
(`ck_open`, `ck_reexport` `not-run`), the screenshot shown, and the manual row recorded.
A T-01 that answers only its echo again is `inconclusive` (R20), as in `game-20261103-09`;
it does not block M6's "end to end", which is about the loop, but the entry says so.

### Part 2: the M7 exit plugin `KC_F31.esp`, made in the window

The active file is new and outside the load order. `set`, `clone` and `place` each write
it once per confirmed OK. `place` refuses a loaded active file, so it goes last and the
file is never loaded before it (the track's order).

| Step | Command (what the window sends) | Expected | Six-status cell |
| ---- | ------------------------------- | -------- | --------------- |
| 2.1 | File > Data...: "Active file path:" `<worktree>\out\f31\KC_F31.esp`, then "Use This File" | the active-file line names it; nothing is written yet (a new file is created by the editor's first write); the CK-shaped list is unchanged (P1: a new, non-master file is unaffected by either rule) | - |
| 2.2 | Records: inspect `Starfield.esm:0021BBCD`; Edit dialog: `FULL` to `KC F31 Marksman Rifle`; OK (`set --dry-run` into the active file), then the confirm (`set`) | the confirm box lists `KC_F31.esp` as the one path changed, the validate findings of the staged result (0 errors) and the limitations; after the write the file exists with one WEAP override. If OK is disabled, record why and stop part 2 (`blocked`) | - |
| 2.3 | Edit > Duplicate on the same WEAP (the editor's `clone`), EditorID `KC_F31_Rifle`; preview, then confirm | one new WEAP in `KC_F31.esp` at its next object ID, its `FULL` as 2.2 left it or as the master's (record which) | - |
| 2.4 | Cell View: Interior, `CityNewAtlantisApartmentTEMPLATE`; Records: inspect `Starfield.esm:1E4FBD` (`Crate_MilitarySupply_01`); Cell View "Place Starfield.esm:1E4FBD here"; position -17, -4, 4.63; preview (`place --dry-run`), then confirm (`place`) | one REFR in the cell's override, temporary (the default, owner decision 2026-11-02); the active file was never loaded | - |
| 2.5 | Close the window. `kit validate --plugin out\f31\KC_F31.esp --plugins Starfield.esm` | 0 errors; warnings listed in the entry | `parse` |
| 2.6 | `kit roundtrip out\f31\KC_F31.esp --verify`; `Get-FileHash -Algorithm SHA256 out\f31\KC_F31.esp` | identical; the SHA-256 recorded in the entry and the G report (C's OW-C launch 2 opens this file) | `byte_roundtrip` |
| 2.7 | `kit testplan out\f31\KC_F31.esp out\f31-plan --plugins Starfield.esm` | `checklist.md` and `probes.toml` (record its SHA-256), with rows for the clone and the placed reference as `kit testplan` writes them (for a reference: `player.moveto`, `prid`, `getisid`, `getpos`), and the persistence row | - |
| 2.8 | Reopen the window; Cockpit: pick `out\f31\KC_F31.esp`; **Review**; **Preview deploy**, **Deploy...** and the confirm (`deploy --dev`) | the dev mod `KitKraft Dev - KC_F31` is made; the window reads it back | `parse`, `byte_roundtrip` (again, from the window) |
| 2.9 | Close the window; `kit deploy enable C:\Modding\MO2-Starfield --mods KC_F31 --dev --profile "Kitkraft Dev"` (`--dry-run` first) | only that mod's lines in the three list files, backed up first | - |
| 2.10 | Reopen the window (`kit ui --dev`): Cell View, `CityNewAtlantisApartmentTEMPLATE` | the placed reference is listed (EditorID, base, position) and draws in the Render Window (STATUS.md: "a placed reference showing in the Cell View and viewport") | - |
| 2.11 | Cockpit: session with `out\f31-plan\probes.toml`; Preview session, Launch..., the poll, Stop... as 1.5 to 1.10 | a second ledger entry from the window: the clone and reference rows graded | `game_runtime` |
| 2.12 | The persistence row of the same session (G3) | `pass`, or `inconclusive` with its reason (an ambiguous `GetInCell` after a retry, `game-20261103-04`) | `save_reload` |
| 2.13 | After: `kit deploy disable C:\Modding\MO2-Starfield --mods KC_F31 --dev --profile "Kitkraft Dev" --dry-run`, then the same without `--dry-run`; the list files hash as before 2.9 | `KC_F31.esp` owns forms, but only `KitKraftSession-*` saves ever held them, and those are disposable | - |

Acceptance of part 2: `KC_F31.esp` SHA-256 recorded; `parse`, `byte_roundtrip` and
`game_runtime` filled; `save_reload` filled by G3; `ck_open` and `ck_reexport` `not-run`
until C's launch 2.

### Part 3: the real-window checks (`plan/STATUS.md`, "To try in a real window", and the 2026-11-07 list)

Each is a note in the writer's reading with what the owner and the agent saw; none is a
six-status cell.

| Check | Where in this sheet | What to record |
| ----- | ------------------- | -------------- |
| KK-032: launch, follow, stop and relaunch | 1.6 to 1.11 | the poll's phases in order; whether the relaunch reused the panel cleanly |
| KK-033: the probe table and screenshots | 1.7, 1.8 | rows, re-runs marked, the screenshot drawn |
| KK-034: the confirm box | 1.4, 1.6, 1.10, 2.2 to 2.4 | one request per click; buttons enabled only while idle |
| KK-036: the Cell View | 2.4, 2.10 | whether the Base column should show the EditorID rather than the formref (the owner's call) |
| KK-040: start-up load timing | 1.1, 2.10 | seconds to loaded, warm or cold |
| a load from a real MO2 profile in the window | 1.1 | the profile named and any limitation line |
| the Object Window's speed | after 2.10: View > Object Windows | seconds to list a large type (WEAP) |
| the viewport's V3 on the dev profile | 2.10 | what the Render Window shows for the apartment cell, and its frame time if shown |
| a placed reference in the Cell View and viewport | 2.10 | listed and drawn, or not |

Then offer the owner M6.O4's sign-off, with the revised T3 (`docs/design/workbench-performance.md`).
Missing evidence is `inconclusive`, never `pass`; a step not reached is `not-run` with why.
