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
     An empty or blank body is not ready; with `ready_expect` empty any non-empty answer
     is, otherwise the answer must contain it;
   - gives up after `ready_secs` from launch (300 s), which covers the window, the settle
     and the polls (the first answer came at 91 s). A profile whose `settle_secs` is not
     below `ready_secs` is an error. The row reasons then name what was still missing (no
     window yet, settling, or the last answer).

   Then it sends `load <base save>` and waits for a non-empty answer again (`load_secs`).
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
- A base save the game does not find (`Save file not found.` after `load <base save>`)
  leaves every row `not-run (the base save did not load: ...)`.
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
