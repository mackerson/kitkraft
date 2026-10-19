# Playbook: a mod from a brief (M4.T10)

This playbook is for an agent that has been told "I want a mod that does X". Follow it
from the top. Every step is a `kit` command, which is one catalog command, so the same
step works from MCP (`execute` / `command_line`). AGENTS.md binds you throughout, and
these rules matter most:

- The owner's profile `Default` is read-only input. It is never a target. Every in-game
  check runs on the MO2 profile `KitKraft Dev`, on disposable saves.
- `kit deploy --dev` is the only way anything gets into MO2. An agent launches the game
  only through `kit game session`, and `kit mod run` calls both for you.
- Report unknown as unknown. A probe that did not run is `not-run`, and missing evidence
  is `inconclusive`. Neither is ever `pass`.

**State (2026-10-16).** Every stage up to the game session works. The session is refused
by preflight with the rule `unverified-profile`, because the Starfield game profile has no
evidence yet for its launch, console and save facts (M4.T1). The console tools are not
installed either (M4.O7, the owner's). So today a run ends `partial` with every probe
`not-run`, and nothing in game is claimed. Once M4.T1 and M4.O7 are done, the same command
runs the session and reports a verdict per probe. You do not need to change anything.

## 1. The brief

Ask the owner for a brief, or write one with them, in this shape. `docs/overhaul/brief.md`
is a full example. Put it at `docs/<mod>/brief.md`. Each point gets an ID that the spec
cites, so the IDs are never reused.

```markdown
# <Mod name> brief

Owner: <who>, <date>. Goal in one sentence: <what the player should see>.

| ID | What (owner's words) | Forms affected (name or EditorID) | Desired change | Measurable target (what a probe or the owner checks) |
| :- | :------------------- | :-------------------------------- | :------------- | :---------------------------------------------------- |
| B1 | ... | ... | ... | e.g. the name reads "X"; capacity equals 7; a crate stands at (-17, -4, 4.63) in cell Y |

## Out of scope
New meshes or textures, animations, voice, anything docs/gaps.md lists as unsupported.

## In-game checklist
| Check | Brief row | Expected | Seen | Pass |
```

Write a measurable target for every row. A row that has none cannot pass.

## 2. Pick the spec kind

Find the forms first. These commands only read: `kit query <TYPE> --where "FULL ~ 'name'"`,
`kit inspect <form>`, `kit refs <form>`, `kit cell --interior`, `kit cell --cell <cell>`.
Then copy the closest spec below and edit it. Put `brief = "B1"` on each entry so the spec
stays traceable to the brief.

| The brief asks for | Spec kind | Start from |
| ------------------ | --------- | ---------- |
| Change fields of one or a few existing records (rename, a stat) | `[[set]]` (spec v1 or v3) | `docs/smoke-rename.toml` (the rename brief) |
| The same change across a class of records (every rifle, every automatic) | v2 classes and `[[rule]]` | `docs/overhaul/overhaul.toml`; `kit overhaul scaffold WEAP --group-by ...` writes a starter |
| An object placed in a cell (a POI) | v3 `[[place]]` | `docs/poi/first-poi.toml` (the first POI spec) |
| An NPC standing in a cell | v3 `[[place]]` of an `NPC_` (an `ACHR`) | `docs/recipes/actors.toml` |
| Who owns a cell | `[[set]]` on `XOWN` | `docs/recipes/ownership.toml` |
| A new outpost buildable | two `[[clone]]` (object and recipe, `new:<name>`) | `docs/recipes/workshop.toml` |
| A new fabricator recipe | `[[clone]]` of a `COBJ` | `docs/recipes/fabricator.toml` |
| A script on a reference | `[[place]]` with `scripts`, and a compiled `.pex` | `docs/recipes/scripts.toml` (compile first, below) |

Each recipe's `.md` beside it says what is known, what is grade D, and what its in-game
check looks like. Read that file before you edit the recipe. If none of these kinds fits,
check `docs/gaps.md`. Say so in the brief rather than inventing a record layout.
AGENTS.md "Clean room" forbids writing a layout from memory.

A plugin that owns new forms (`[[place]]`, `[[clone]]`, `[[new]]`) is for disposable saves
only. The run's report and the deploy step say so. Override-only plugins (`[[set]]`, rules)
are safe to remove from a save.

Check the spec before a run. Both of these write nothing:

```powershell
kit build <spec> out\<Name>\<Name>.esm --plan     # every change, before and after, and the validation result
kit mod run <spec> --name <Name> --dry-run        # build --plan, plus every stage's arguments
```

## 3. Run it

**Before the first run, select the `KitKraft Dev` profile in MO2** (the profile drop-down
at the top of its window), then close MO2 so `ModOrganizer.ini` records it. The deploy
stage reads MO2's active profile and never deploys while it is `Default`, the owner's live
game; the run then ends `fail` at stage 5 with this refusal:

```text
refused [live-profile] C:\Modding\MO2-Starfield\ModOrganizer.ini: MO2's active profile is `Default`, the owner's live game; switch MO2 to the `KitKraft Dev` profile first
```

(If `ModOrganizer.ini` names no profile at all, the deploy stage accepts the dev profile
`kit mod run` already passes it, `--dev-profile` (default `KitKraft Dev`), when that
profile exists; only when it does not is the refusal `[live-profile-unknown]`: create the
profile, name an existing one with `--dev-profile`, or open MO2 on it once. `--profile`
is the load order's MO2 profile, read only, not the one deploy checks.)

```powershell
kit mod run docs\smoke-rename.toml --name OldSovietMarksmanRifle --mo2 C:\Modding\MO2-Starfield --base-save-sha256 <hex>
```

`<hex>` is the SHA-256 of the base save `KitKraftBase`, which the owner records at M4.O7.
Leave it out until then; the session is refused anyway. The run's folder is
`out\<name>\` (`--out` moves it), and the chain is:

| # | Stage | Command it runs | Writes |
| - | ----- | --------------- | ------ |
| 1 | build | `build <spec> out\<name>\<plugin> --overwrite` | the plugin |
| 2 | validate | `validate --plugin <plugin>` (load order: the spec's `[load_order]`) | nothing |
| 3 | roundtrip | `roundtrip <plugin> --verify` | nothing |
| 4 | package | `package <plugin> out\<name>\package --overwrite` (`--scripts`, `--loose`, `--ba2` pass through) | the mod folder and zip |
| 5 | deploy | `deploy --dev out\<name>\package\<stem> --mo2 <mo2>` | the MO2 mod `KitKraft Dev - <stem>` only (ADR 0009) |
| 6 | testplan | `testplan <plugin> out\<name>\testplan` | `kitkraft-test.txt`, `checklist.md`, `probes.toml` |
| 7 | session | `game.session --dry-run` (preflight), then `game.session` when nothing is refused | `observe\<date>-T<nn>\`, one ledger entry (ADR 0010) |

The options are these:

- `--until <stage>` stops after that stage, and the later stages are `not-run`. Use
  `--until testplan` to stop short of the game.
- `--dry-run` writes nothing.
- `--plugin <file>` sets the plugin name. The default is `<name>.esm`.
- `--dev-profile` defaults to `KitKraft Dev`.
- `--game`, `--base-save`, `--observe` and `--ledger` are passed to `game.session`.
- `--profile`, `--plugins` and `--data-dir` override the spec's load order.

Rerunning the same command reuses `out\<name>\`, which is the fix loop. A folder that a mod
run did not write is never reused.

**Enabling the mod.** Deploy never edits a profile, and nothing switches MO2's active
profile: `KitKraft Dev` must already be selected in MO2 before the first run (above;
otherwise stage 5 ends `fail` with the `refused [live-profile]` refusal quoted there).
After the first deploy, `KitKraft Dev - <stem>` must be enabled in the `KitKraft Dev`
profile with its plugin ticked last. When the owner asks for it in the session, close MO2
and the game and run `kit deploy enable --dev <mo2> --mod <stem>` (ADR 0009 Amendment 2,
`docs/deploy.md`), or the owner does it in MO2 by the deploy stage's `next_steps`. Until
then, preflight refuses with `plugin-not-deployed`. The mod stays enabled across
redeploys.

**Scripts recipe.** Compile into the run's folder before the run, so the build finds the
`.pex` next to the plugin. Then pass the scripts to package:

```powershell
kit papyrus compile docs\recipes\scripts\KitKraft\HelloOnLoad.psc out\KitKraftScripts\Scripts --sources-zip "<Starfield>\Tools\ContentResources.zip"
kit mod run docs\recipes\scripts.toml --name KitKraftScripts --scripts out\KitKraftScripts\Scripts --mo2 C:\Modding\MO2-Starfield
```

## 4. Read the report

`out\<name>\report.md` is written for people and agents, `report.json` holds the same data,
and `stages\<nn>-<stage>.json` holds each stage's full envelope with its evidence. The
report has these sections:

- **Result.** `pass` means every stage and every probe passed. `fail` means a stage or a
  probe failed; then `data.passed` is `false` and `kit` exits 1. `partial` is everything
  else: a refused session, `--until`, an `inconclusive` row, or a `manual` row. The result
  is read off the rows and never chosen beside them (AGENTS.md "Reporting").
- **Stages.** Each stage's status, its summary's first line, its arguments and
  limitations, and its envelope file. The run stops at the first failing stage, and the
  stages after it say `not-run (stopped at the failing <stage> stage)`.
- **Session.** Either the preflight refusals (rule and reason), or the session id, its
  status and the ledger entry (`game-<yyyymmdd>-<nn>` in `docs/game-observations.md`).
- **Probes.** One row per `probes.toml` row (`T-<nn>`), with `pass`, `fail`, `inconclusive`
  or `not-run` and the reason.
- **Next steps.** What to fix, or what is missing (owner steps named as such).

To hand off a ticket, copy the YAML fields from the report:

| YAML row | Comes from |
| -------- | ---------- |
| `parse` | build and validate |
| `byte_roundtrip` | roundtrip |
| `ck_open`, `ck_reexport` | `not-run` (`kit mod run` does not drive the CK) |
| `game_runtime` | the probes other than persistence |
| `save_reload` | the persistence probe (`persistence 1-3`) |

## 5. The fix loop

Fix the first failing stage, then rerun the same `kit mod run`.

| Stage fails | What it means | Fix |
| ----------- | ------------- | --- |
| build | The spec does not parse, a form is not found, a value is refused, or the new plugin does not validate | Read the error in the stage row. Check forms with `kit query` / `kit inspect`, and check the spec against `kit build --plan` |
| validate | A rule found an error (missing master, bad link, copied string ID, unknown script) | `kit validate --plugin <plugin>` lists findings with offsets; `docs/validate.md` explains each rule |
| roundtrip | The written plugin does not reread byte for byte | This is a writer bug, not a spec bug. File it under `plan/STATUS.md` "Requests" with the envelope file |
| package | Validation inside package failed, or `--scripts` / `--loose` paths are wrong | Read the stage summary |
| deploy | `mo2` was missing, the source changed, or a refusal rule fired (`deploy --dry-run` names them) | Never work around a deploy refusal by copying files by hand |
| testplan | The plugin or its load order could not be read | Check the spec's `[load_order]`, or pass `--plugins` |
| session `not-run` (refused) | Preflight said no; each rule is in the report | Fix what is yours to fix (`running`: close the game or MO2; `plugin-not-deployed`: ask the owner to enable the mod). `unverified-profile`, `console-plugin`, `base-save` and `ini-switch` wait on M4.T1 and M4.O7 |
| session `fail` / `inconclusive` | A probe ran and saw something else, or saw nothing | See the next table |

What a failing probe means, by spec kind:

| Spec kind | Probe | `fail` usually means | `inconclusive` usually means |
| --------- | ----- | -------------------- | ---------------------------- |
| `[[set]]` rename or field change | `help "<FULL>" 4` contains the runtime ID | The plugin is not active or not last, a later plugin overrides the form, or the runtime ID was predicted for another load order (`runtime_id_basis` in `probes.toml`; confirm with `testplan --from-save`) | The console printed nothing (an endpoint or command issue, grade D until M4.T1) |
| v2 rules (overhaul) | One `help` row per changed record (by its FULL, else its EditorID). Numbers such as damage and capacity are not probed | As above, for each record. A number that is wrong in game is a `manual` row the owner checks against the brief's checklist | As above |
| `[[place]]` POI / object | `coc` + `player.getincell`; `prid` + `getpos x/y/z` against `DATA`; `isdisabled` | The reference loaded elsewhere: the cell or position is wrong (a world position means another cell), or the plugin is not loaded, so `prid` finds no EditorID | `prid` printed nothing; a screenshot row waits for the owner (Amendment 1, condition 8) |
| persistence (every kind) | `save {save}`, `load {save}`, `player.getincell` | The save did not load where it was made | Save or load is grade D until M4.T1 (d) |
| actors (`ACHR`) | `player.moveto`, `getisid` on the base | A different base, or the NPC was not placed (the cell is not loaded) | Sandboxing is a screenshot for the owner |
| ownership, workshop, fabricator | Mostly `manual` (prompts, menus, crime are UI) | Not applicable | `manual` rows stay `not-run` with their reason; the owner closes them |
| scripts | A `papyrus-log` row once M4.T6 emits it | The script did not run (wrong event, not packaged: did you pass `--scripts`?) | The Papyrus log was not written (INI switches, M4.O7) |

Never mark a probe `pass` by hand. If the controller could not close a row, it stays
`manual` or `inconclusive`. The owner decides (Amendment 1, condition 9).

## Scripts (PowerShell, `scripts\`)

These are thin wrappers over `kit`, and the CLI stays the one implementation:

- `scripts\launch-dev.ps1 -Probes out\<name>\testplan\probes.toml -Mo2 <mo2> [-BaseSaveSha256 <hex>]`
  prints the MO2 command line that `kit game session` would start. Until
  `docs/game-observations.md` records the launch fact (M4.T1 (a)), it refuses to run it:
  the game profile's `[launch] evidence` must name a whole ledger entry id
  (`game-<yyyymmdd>-<nn>`) whose heading the ledger holds. After that, it runs
  `kit game session`, the only launcher an agent may use. It runs this checkout's
  `bin\kit.exe` (else `dist\KitKraft\kit.exe`), never whatever `kit` is on PATH, and
  stops with exit 2 when neither exists.
- `scripts\tail-console.ps1 (-Probes ... -Mo2 ... [-Wait] | -Session <id>)` tails the
  console history log, either the live one or the copy a session made.
- `scripts\open-observe.ps1 [-Session <id> | -Report out\<name>\report.json]` opens a
  session's `observe\` folder.

## Check (transcript)

`crates/engine/tests/mod_from_brief.rs` runs this playbook's step 3 command line for the
rename brief (`docs/smoke-rename.toml`) and for the first POI spec
(`docs/poi/first-poi.toml`). The command line is parsed by the CLI grammar and executed as
`kit` does it, against a fixture MO2 instance. Stages 1 to 6 pass, and the rename
plugin's SHA-256 is `docs/smoke-rename.md`'s. The session is refused with
`unverified-profile` (plus `running` when MO2 or the game happens to be open), and every
probe is `not-run`. The result is `partial` with exit 0. That is where "a fresh agent
session produces the rename plugin and a pass without an owner step" stops today: it waits
for M4.T1 and M4.O7. `crates/engine/src/handlers/modrun_tests.rs` runs the same chain on
synthetic data, including a session against the fake console. It produces a ledger entry,
`pass` on the rename and persistence probes, and `fail` when the console shows another ID.
