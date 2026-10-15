# Agent-driven in-game verification: where we stand and the state of the art

Date: 2026-10-15. Research note (not an ADR; the decisions it raises are listed at the end
for the owner). Sources are linked per fact; anything observed on this machine says so.

## 1. Goal

Close the QA loop without the owner: an agent builds a plugin with KitKraft, launches
Starfield on the MO2 profile `KitKraft Dev`, drives the game to the test cell, checks what
the plugin should have changed, records the result in `docs/game-observations.md`, and
tears the session down. Later: other Bethesda titles.

## 2. What the codebase already has (observed in the repo at `main`)

| Piece | What it does today | Gap for an unattended agent |
| ----- | ------------------ | --------------------------- |
| `kit testplan` (`crates/engine/src/handlers/testplan.rs`) | Writes `kitkraft-test.txt` (a console batch: `coc`, `player.moveto`, `prid`, `player.additem` with runtime IDs computed for the dev load order) and `checklist.md` with expected observations and the persistence protocol | The batch is copied and run by a human; expectations are prose, not machine-checkable probes; nothing reads results back |
| `docs/game-observations.md` | Ledger format: per-row Step / Forms / Expected / Seen / Result / Evidence, persistence protocol (leave, wait, save, reload, return, long absence) | Filled by hand. The format is already what an agent should emit |
| `kit deploy --dev` (ADR 0009) | Installs a package into one KitKraft-managed MO2 mod folder; refuses `Default` | Must run with the game closed; nothing launches the game afterwards |
| `kit papyrus events`, scripts recipe, `validate` attached-script rules | Static side of Papyrus | No runtime channel: `Debug.Trace` output is never read |
| ADR 0008 | Runtime extensions (SFSE plugins) permitted when a runtime-need record justifies them; core never links them; CommonLibSF unread until licence review; `Starfield.exe` never debug-attached or disassembled | Launching the game and talking to a third-party SFSE plugin over HTTP is not covered either way: it is neither forbidden nor planned |
| AGENTS.md "Owner's live game is off limits" | `Default` never a target; `KitKraft Dev` + disposable saves only | Written on the assumption that **the agent cannot launch the game** (`docs/smoke-rename.md`, testplan header). That assumption is the thing to revisit |

Facts on this machine (2026-10-15): `Starfield.exe` 1.16.244 (Steam), SFSE 0.2.21 live
(`Documents\My Games\Starfield\SFSE\Logs\sfse.txt`, run of 2026-10-15); the game already
writes every console command to `Documents\My Games\Starfield\StarfieldConsoleHistory.log`
(commands only, no output); `KitKraft Dev` has profile-local saves from a session on
2026-10-15 (`profiles\Kitkraft Dev\saves\`); `Cassiopeia Papyrus Extender` and
`Address Library for SFSE Plugins` are installed in the MO2 instance; neither profile's
`StarfieldCustom.ini` enables Papyrus logging.

## 3. What an unattended loop needs, and what exists for each channel

### 3.1 Launch into a known state

- **MO2 launch.** MO2 can start an executable on a named profile from the command line
  (`ModOrganizer.exe -p "Kitkraft Dev" "SFSE"` style; exact switch to verify against MO2
  2.5.2's `--help`). MO2's VFS is per launch, so the dev profile's mods are what the game
  sees. Grade D until tried.
- **Window and focus.** The game pauses when unfocused unless `bAlwaysActive=1` under
  `[General]` in the profile's `StarfieldCustom.ini` (documented on the Console API mod
  page, below). Windowed/borderless via `StarfieldPrefs.ini` (`bBorderless`,
  `bFull Screen`, `iSize W/H`) ([Steam guide](https://steamcommunity.com/app/1716740/discussions/0/3824173464663416259)).
- **Run commands at boot.** `[General] sStartingConsoleCommand=bat autoexec` in
  `StarfieldCustom.ini` runs a batch at launch, no plugin needed
  ([Nexus 607](https://www.nexusmods.com/starfield/mods/607),
  [Nexus 4687](https://www.nexusmods.com/starfield/mods/4687)). Community-documented
  only; grade C.
- **Load the disposable save.** No "start on save" mod for Starfield was found. Candidates:
  the console `load <save>` command from the main menu (works in earlier Creation Engine
  games; grade D for Starfield), driven either by `sStartingConsoleCommand` or over HTTP
  (3.2); or input injection into the menu (3.4). Console Command Runner's `GameLoaded`
  event then fires the test batch once the world is live.

### 3.2 Execute commands from outside the game

| Tool | Channel | Returns output? | Licence / source | Maintained | Notes |
| ---- | ------- | --------------- | ---------------- | ---------- | ----- |
| **Console API and Web Application** (stonegdi, [Nexus 4280](https://www.nexusmods.com/starfield/mods/4280)) | SFSE plugin; `POST http://127.0.0.1:55555/console` with the command as the body | **Yes**: echoes the command and the console's untruncated output; `iExecTimeout` waits for trailing lines; optional file output | Source on GitHub ([dranger003/sfse_plugin_console_api](https://github.com/dranger003/sfse_plugin_console_api)), **no licence file** (all rights reserved by default): run as an installed tool only, never copy; C++, Boost.Beast HTTP | v2.17.0, 2026-06-12 (36 commits) | Declares `kAddressIndependence_Signatures` in its SFSE version block (observed in `src/plugin.cpp`), so it signature-scans rather than binding to one game version: likely to survive 1.16.244, **unverified** |
| **Console Command Runner Updated** (Bobbyclue, Meridiano, [Nexus 7318](https://www.nexusmods.com/starfield/mods/7318)) | TOML event files in `Data\SFSE\Plugins\ConsoleCommandRunner\`; events `DataLoaded`, `GameLoaded` (new game / save loaded), `OnMenuOpenCloseEvent`, `OnKeyPress`, `OnItemEquipped`, `OnQuestStage`; `{Form:plugin:0xID:default}` text replacement resolves runtime FormIDs | No (fire-and-forget) | GPL-3.0 source ([Bobbyclue/Console-Command-Runner-SF](https://github.com/Bobbyclue/Console-Command-Runner-SF), CommonLibSF template): run only, never read into `crates/*` | v1.11.0, **2026-10-14** | Its FormID replacement makes the batch load-order independent, which `kit testplan` currently computes itself (`runtime-slots-by-scale`, grade D). Also a way to cross-check that assumption |
| `sStartingConsoleCommand` | INI | No | none | n/a | Boot-time only |
| Console Command Center ([JerKnuts](https://github.com/JerKnuts/Console-Command-Center), Nexus 18359) | In-game web UI (OSF UI + WebView2) over a native plugin; batch `.txt` files | In-game only | GPL-3.0-or-later with CommonLibSF modding/linking exceptions | v1.2.35, runtime 1.16.244 | Human-facing; no external interface |
| Modern Command Framework ([Bassdoubs](https://github.com/Bassdoubs/ModernCommandFramework)) / Snapdragon's Custom Console Framework ([Nexus 9779](https://www.nexusmods.com/starfield/mods/9779)) | Register new console commands from a plugin | n/a | CommonLibSF-based | - | Only relevant if KitKraft ships its own runtime component (ADR 0008 §5) |

### 3.3 Read results back

- **Console output to file.** stonegdi's **Console Output To File** ([Nexus 3142](https://www.nexusmods.com/starfield/mods/3142), v1.21, 2026-06-12) writes the
  full console text to `Data\SFSE\plugins\sfse_plugin_console.log` (lands in MO2
  `overwrite\`); the Console API plugin does the same with a configurable path. Either
  gives a tail-able transcript; the HTTP response is the synchronous form.
- **Papyrus log.** `[Papyrus] bEnableLogging=1 bEnableTrace=1 bLoadDebugInformation=1` in
  the profile's `StarfieldCustom.ini` (MO2: the profile copy, not `My Games`) writes
  `My Games\Starfield\Logs\Script\Papyrus.0.log`
  ([Nexus article 780](https://www.nexusmods.com/starfield/articles/780)). This is the
  channel for the scripts recipe (`Debug.Trace` from an attached script) and for errors
  the engine logs when a plugin's script is missing or mis-typed.
- **Console probes with textual answers** (the oracle vocabulary): `help <text> 4` (lists
  forms whose name matches: a direct check of a renamed `FULL`), `prid` + `getpos x|y|z`,
  `getav`, `GetStage`, `IsDisabled`, `GetDistance`, `DumpFormList`, `DumpPapyrusStacks`,
  `CGF "Script.Function"` / `CF` to call Papyrus (Cassiopeia adds ~115+ native functions,
  [Nexus 11097](https://www.nexusmods.com/starfield/mods/11097)). The `refinfo` batch
  pattern (a Papyrus global printing position, rotation, owner, parent cell to the
  console) is documented on [Nexus 607](https://www.nexusmods.com/starfield/mods/607).
- **Console history.** `StarfieldConsoleHistory.log` (observed): commands only, useful to
  prove what ran.

### 3.4 See the screen and press keys

- Pixels are the fallback, not the primary oracle (see §4). Options: the desktop
  computer-use tools available to Claude in this app (screenshot, click, type), or
  open-source harnesses: **GamingAgent** (lmgame-org, MIT, ICLR 2026, "computer-use
  gaming agents on local PCs", [GitHub](https://github.com/lmgame-org/GamingAgent)) and
  **gaming-mcp** (Apache-2.0; DXGI capture, `SendInput` scan codes, ViGEmBus virtual pad,
  perceptual-hash frame skipping, MCP transport;
  [GitHub](https://github.com/David-BOOM/gaming-mcp), early-stage: 0 stars).
- In-game screenshots: Steam F12 or the game's own key, saved to a known folder; the
  ledger already expects files under `observe\<date>-<session>\`.

### 3.5 Prior art closest to this exact goal

- **skytest** in [myaiexp/skyrim-headless-mods](https://github.com/myaiexp/skyrim-headless-mods)
  (0BSD, Skyrim SE, Linux/wine, 288 commits, built with `AGENTS.md`/`CLAUDE.md` for agent
  use). The design to borrow: a passive **probe plugin** armed at runtime; an external
  controller writes JSON commands to `commands.jsonl`, the game writes structured
  `trace.jsonl` (`trace`, `dump`, `watch`, `set-av`, `ui-get`...); `Data/` swapped
  between symlinked profiles (vanilla / vanilla+mod / full) for A/B isolation; the Saves
  folder isolated to one test save; `skytest replay` runs saved `.steps` files as
  regression tests; `--headless` under gamescope. Its "boot into save" depends on a
  third-party Start-On-Save mod and is broken on the current Skyrim build, which is the
  same open problem as our 3.1.
- **AutoTest** ([Muriel-Salvan/AutoTest](https://github.com/Muriel-Salvan/AutoTest),
  BSD-3, Skyrim SE; FO4 untested). Suites as Papyrus scripts (`NPCs`, `NPCsHead`,
  `Locations`: teleport, orbit camera, screenshot, detect CTD); tests listed in JSON run
  files; per-suite **Statuses JSON updated in real time so an external process can read
  progress**; results also in `Logs\Script\User\AutoTest.*.log`; driven by console
  `start_tests` / `stop_tests` and by the CLI tool Modsvaskr. Needs ConsoleUtil,
  PapyrusUtil, SkyUILib.
- **papyrus-debug-server** ([joelday](https://github.com/joelday/papyrus-debug-server)):
  Debug Adapter Protocol over websocket for Skyrim/FO4 Papyrus. No Starfield port found.
- **Mantella / CHIM / AISS** pattern (script-extender plugin plus an external Python
  server over HTTP or files) shows the bridge shape is routine in this community
  ([Mantella](https://github.com/art-from-the-machine/Mantella),
  [HerikaServer/CHIM](https://github.com/language-coder/HerikaServer)).
- **Skyrim Creation Kit MCP** (Pyrhame, April 2026,
  [PulseMCP](https://www.pulsemcp.com/servers/pyrhame-skyrimck)): plugin-file editing
  via Claude, not a running game. Same category as KitKraft's MCP; no overlap with this
  note.

## 4. What the research says about LLM agents as game testers (2024-2026)

- **GBQA** (April 2026, [arXiv 2604.02648](https://arxiv.org/html/2604.02648)): 30
  games, 124 verified bugs. Agents observe through **backend state APIs and text, not
  screenshots**; actions are API endpoints as tools in a ReAct loop. "QA mode" (agent
  gets design docs and source) beat free exploration for every model; recall still under
  50% at the top (Claude Opus 4.6 thinking, 500 steps: 48.4%). More steps keep finding
  harder bugs; memory across sessions matters; failures come from state-tracking drift
  over long horizons and the lack of QA heuristics. A closed loop with Claude Code as the
  patcher found and fixed all three bugs of one environment in three sessions.
- **TITAN** (Sept 2025, [arXiv 2509.22170](https://arxiv.org/abs/2509.22170)): LLM agent
  for commercial MMORPG QA; abstracts state dumps into text templates, prioritises
  actions, keeps an action-trace memory, and runs **crash, logic/stall and performance
  oracles** that emit reports with causal traces, screenshots and repro steps; reported
  in eight production QA pipelines (authors' claim).
- Survey ([Emergent Mind](https://www.emergentmind.com/topics/llm-agents-as-game-testers)):
  across TITAN, Lap (match-3 playtest), SMART (Overcooked regression), GAMA, Xiao et al.:
  observation channels are numeric state, structured dumps, or vision pre-processed into
  symbols; **no system relies on raw screenshots as its oracle**; screenshots appear in
  diagnostic reports. Limits: latency for real-time play, weak visual perception,
  non-reproducible outputs, context limits over long runs.
- Bethesda itself: no public automated-agent QA; Todd Howard describes AI as a
  development aid only ([Windows Central](https://www.windowscentral.com/artificial-intelligence/bethesdas-todd-howard-says-you-cant-ignore-ai-tools-but-notes-were-not-using-it-to-generate-anything-for-the-elder-scrolls-6)).

**Takeaway.** The field converged on the shape KitKraft already has on the authoring side:
text and structured state in, deterministic probes, an explicit oracle per expectation,
screenshots as evidence. The missing piece is purely the runtime channel.

## 5. Recommended shape: a `kit game` lane

Everything below respects ADR 0001 (no debug-attach, no disassembly), ADR 0008 (core never
links a runtime framework; third-party plugins are run as installed tools, never read into
`crates/*`), and AGENTS.md (`KitKraft Dev` only, disposable saves).

1. **Probes replace prose.** `kit testplan` gains a machine-readable `probes.toml` next to
   `checklist.md`: each row = a console command (or short batch) + a matcher on the
   returned text (`help "Old Soviet Marksman Rifle" 4` contains `WEAP ... (0021BBCD)`;
   `getpos x` within tolerance of the placed `REFR` position; `GetStage` equals N;
   Papyrus-log line present). Expected values come from the plugin KitKraft just wrote,
   so the oracle is derived, not hand-typed. Pass/fail is a string or number comparison.
   A vision model is used only for "does the scene look sane" annotations attached as
   evidence.
2. **Session controller** (new top-level `runtime/` tool or an `RT`-layer crate per ADR
   0008 §1; it talks to files and HTTP, never to the process): snapshot the dev profile;
   write the INI switches (`bAlwaysActive`, windowed, Papyrus logging) into the **profile**
   copy only; launch SFSE through MO2 on `Kitkraft Dev`; poll `127.0.0.1:55555` until the
   Console API answers `GetSFSEVersion`; load the disposable save (console `load`, grade
   D, else input injection); run the probes; collect HTTP responses, the console log,
   `Papyrus.0.log`, and screenshots; write the `game-<date>-<nn>` ledger entry with every
   row filled (a probe that could not run is `not run` with the reason, as the ledger
   already demands); terminate the game; restore the profile. Hard rules: refuse if MO2's
   `selected_profile` is `Default`; refuse if `Starfield.exe` is already running (the
   owner may be playing); never touch `profiles\Default\`.
3. **Dependencies as installed tools, in the dev profile only:** Console API (HTTP +
   output), Console Output To File (optional, redundant with the API's file output),
   Console Command Runner Updated (`GameLoaded` auto-run and FormID replacement). All
   downloaded from Nexus by the owner (the in-app browser is logged in; downloads need an
   explicit OK). Their licences (none / GPL-3) make them run-only, exactly like Spriggit.
4. **A KitKraft probe plugin later, only by runtime-need record.** If the console
   vocabulary proves insufficient (for example reading an OMOD-applied weapon stat), the
   skytest pattern (passive plugin, `commands.jsonl` in / `trace.jsonl` out) is the design
   to copy. Writing it requires either CommonLibSF (blocked on the ADR 0008 §3 licence
   review) or our own signature scanning, which collides with the clean-room rule: this is
   why third-party plugins as tools come first.
5. **Verification ladder.** V0: HTTP `GetSFSEVersion` answers on `Kitkraft Dev` (proves
   the plugin works on 1.16.244). V1: the rename check (M2.O6) reproduced unattended via
   `help`. V2: the first POI (M4.O2 rows O2-05..O2-08) via `coc`/`moveto`/`getpos` plus the
   persistence protocol driven by `load`/`save` console commands. V3: overhaul build 2
   rows with whichever stats have a console or Papyrus getter. V4: replayable regression
   (skytest `.steps` idea) run after every `main` merge that changes a recipe.
6. **Other titles.** The same architecture transfers: SKSE/F4SE have ConsoleUtil-class
   plugins and the same INI and Papyrus log conventions; AutoTest and skytest already
   exist for Skyrim SE. The session controller should be game-profiled (framework, port,
   log paths, save-load command) from day one.

## 6. Risks and unknowns (each a grade-D fact until observed)

- Console API compatibility with 1.16.244 (last release 2026-06-12, before the Free Lanes
  update; signature-based, so probably fine).
- Loading a save from the main menu by console command.
- MO2 command-line launch of a named profile and executable; MO2 must not already be open
  on another profile.
- Cycle time: launch plus save load is minutes, so probes batch per session, and the
  controller must detect a hang (no HTTP answer, no console log growth) and kill the
  process.
- GPU and Steam are shared with the owner's own play; one game instance at a time.
- `kAddressIndependence_Signatures` plugins can still break on a patch; the controller
  reports "plugin absent" (V0 fails) rather than guessing.

## 7. Owner decisions this raises

1. Reverse the "agent cannot launch the game" assumption for `Kitkraft Dev` only, with the
   refusal rules in §5.2 (an AGENTS.md amendment).
2. Approve installing Console API (+ Console Output To File, Console Command Runner
   Updated) into the dev profile, run-only, licences noted in `docs/source-registry.md`.
3. Whether screenshots / desktop input injection are in scope for V2+ or deferred.
4. Whether a probe plugin of our own (§5.4) may be planned under ADR 0008 before the
   CommonLibSF licence review, or must wait for it.
