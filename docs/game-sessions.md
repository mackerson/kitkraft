# Game sessions (`kit game session`, ADR 0010)

The session controller lets an agent run a plugin's in-game checks on the MO2 profile
`KitKraft Dev` (folder `Kitkraft Dev`) with no owner step. It launches Starfield through
MO2 and drives it over the Console API plugin's loopback HTTP endpoint. It checks the
probes `kit testplan` wrote, records what it saw in `docs/game-observations.md`, and then
ends the game. The rules are ADR 0010 and its Amendment 1. The code lives in `crates/game`
(L3) and in the engine commands `game.session`, `game.status`, `game.probe` and
`game.stop`.

**State (2026-10-16): built and tested against fakes; no real session has run.** The
Starfield profile has no evidence yet for its launch, console and save facts (M4.T1), so
the controller refuses a live session. The console tools are not installed either
(M4.O7). Every runtime row stays `not-run` until both are done. `--dry-run` works now.

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
implementation. A front end that stays open follows a session through `game.status`, and
it needs no second engine worker. A started session belongs to the engine that started
it. When that engine goes away, the session is stopped properly: the game is ended, the
profile verified and the ledger entry written. With the one-shot `kit` CLI that happens
as soon as the command returns, so use the synchronous form there and `--start` from MCP
or the window.

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
| `base-save` | `saves\<base>.sfs` is missing, its SHA-256 was not given, or it differs |
| `plugin-not-deployed` | the plugin of `probes.toml` is not a `kit deploy --dev` mod (`KitKraft Dev - <stem>` with `kitkraft-deploy.json`) enabled in the profile |
| `unverified-profile` | the game profile has a fact without evidence (below) |

Two of these show what the dev box needs from the owner (M4.O7). First, on 2026-10-16
`Kitkraft Dev\settings.ini` says `LocalSettings=false`. With that setting the game reads
the INIs under `Documents\My Games\Starfield`, not the profile's. Profile-specific game INI
files must be switched on in MO2 before the profile's switches take effect. Second, the
profile's `StarfieldCustom.ini` has none of the switches yet. The controller only reads
INIs and never writes them.

## A session

1. Back up `modlist.txt`, `plugins.txt`, `loadorder.txt`, `lockedorder.txt`, the
   profile's `*.ini` and the base save into `observe\<id>\backup\`, with their SHA-256.
2. Start `ModOrganizer.exe -p "<profile>" "moshortcut://:SFSE"`. This command line is
   grade D until M4.T1 (a) observes it (`[launch]` in the profile).
3. Poll the readiness command (`GetSFSEVersion`) until the endpoint answers, then
   `load <base save>` and wait for it to answer again.
4. Run the probes in order. A row has its own deadline (`probe_secs`), and the session
   has a wall-clock budget (`session_secs`). The controller sends no `save` or `load` to a
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
runtime_id_basis = "confirmed-by-save"

[[probe]]
row = "T-04"
title = "prid KitKraftFirstPOICrate; getpos x/y/z: ..."
forms = ["KitKraftFirstPOI.esm:001000", "KitKraftFirstPOICrate", "04001000"]
screenshot = true

[[probe.step]]
command = "prid KitKraftFirstPOICrate"

[[probe.step]]
command = "getpos x"
matcher = "number"
expected = "-17"
tolerance = 0.5
why = "the reference's DATA position in the plugin"

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
| `number` | The last number in the output is within `tolerance` of `expected` |
| `papyrus-log` | The Papyrus log written during the session contains `expected` |

A step without a matcher is setup. `{save}` stands for the session's own save name and
`{base_save}` for the base save's name. What the console prints for each command is
grade D until M4.T1 observes it.

## Game profiles (M4.T9)

`crates/game/profiles/<game>.toml` lists everything the controller knows about one game,
and nothing in it is read from a game binary.

| Section | Keys |
| ------- | ---- |
| `[game]` | `id`, `name`, `status` (`verified`, `partial`, `unverified`), `note` |
| `[executables]` | `game`, `script_extender`, `editor`, `mod_manager`, `helpers` (file names) |
| `[launch]` | `mo2_executable` (the MO2 executable title), `args` (`{profile}`, `{executable}`), `evidence`, `pending` |
| `[console]` | `url` (loopback `http://` only), `ready_command`, `ready_expect`, `mod_names`, `candidates`, `evidence`, `pending` |
| `[saves]` | `dir` (in the profile), `extension`, `load_command`, `save_command` (`{save}`), `base_default`, `session_prefix`, `evidence`, `pending` |
| `[mo2]` | `local_settings`, `local_saves` (required `settings.ini` values) |
| `[[ini]]` | `file`, `section`, `key`, `value` |
| `[[log]]` | `name`, `path` (`{documents}`), `evidence`, `pending` |
| `[timeouts]` | `session_secs`, `probe_secs`, `ready_secs`, `load_secs`, `exit_secs` |

`evidence` names the ledger entry that observed a fact. While `launch`, `console` or
`saves` has none, the controller refuses a live session and names each missing fact with
its ticket. Logs without evidence do not block a session; a log that is missing makes its
rows `inconclusive`.

| Profile | Status | Missing |
| ------- | ------ | ------- |
| `starfield` | partial | launch (M4.T1 a), console (M4.T1 b, V0), saves (M4.T1 d) |
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
| O2-05 | probe | POI `T-01`: `coc <cell>`, `player.getincell <cell>` |
| O2-06 | probe | POI `T-02` (`player.moveto`, `getdistance player`) and `T-03` (`help <EditorID> 4` contains the runtime ID) |
| O2-07 | probe | POI `T-04`: `prid`, `getpos x/y/z` against `DATA`, `isdisabled`, the numeric ID; screenshot |
| O2-08 | probe | POI `T-05` (leave, `save {save}`, `load {save}`) and `T-06` (return, `getpos` again) |
| O2-09 | manual | the long absence (168 game hours) has no chosen console route (FTH-EXP-07) |
| O2-10 | manual | `placeatme` makes a reference whose ID is known only at run time (PS-09) |
| O2-11 | probe | POI `T-03`: `help KitKraftFirstPOICrate 4` |
| O2-12 | manual | the map and location UI |
| O2-13 | manual | `n/a` for packages without an archive |
| O4-01 | manual | the "Take" or "Steal" prompt is UI; the ownership plugin's probes cover persistence only |
| O4-02 | manual | crime and bounty are UI |
| O4-03 | probe | actors `T-02` (`player.moveto`) and `T-03` (`getisid` on the base); the sandboxing walk is a screenshot for the owner |
| O4-04 | probe | actors persistence rows (`save {save}`, `load {save}`, `getpos` within tolerance) |
| O4-05 | manual | the outpost build menu is UI |
| O4-06 | manual | the fabricator is UI |
| O4-07 | manual | until the scripts recipe's testplan emits a `papyrus-log` probe for its `Debug.Trace` line (M4.T6); the matcher exists |
| O4-08 | manual | a procedural landing (M4.E10) |
| O4-10 | manual | needs the M4.E9 script recipe (PS-06) |
| O4-11 | manual | needs M4.E9 (PS-09) |
| O4-12 | manual | needs M4.E9 (PS-10) |
| O4-13 | manual | `disable` is not in the probes `kit testplan` writes; a hand-written `probes.toml` can do it with `isdisabled` |
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
