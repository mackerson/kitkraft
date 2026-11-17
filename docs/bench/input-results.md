# Input injection: results of 2026-11-16 (unattended)

The first trial of desktop input under ADR 0010 Amendment 2 (the external executor the
bench sheet's input block names), run unattended by an agent on 2026-11-16 from 03:51 to
04:12 on the owner's go of that night. This page summarises it for the repo. The scripts
and every result file live **outside the repo**, in
`C:\Modding\kitkraft-observe\scripts\input\`, which is **not under version control** (a
copy of the folder before that night's script fixes is `..\input-backup-20261116`).

| Source | SHA-256 |
| ------ | ------- |
| `RESULTS-20261116.md` (the run's own write-up) | `2ad508f16925bdec159d8eedbc32789a971a4a169d7c749c18fa882e06ff9f46` |
| `results-20261116.json` (machine summary: runs, findings, measurements) | `ef1b07176dc36beabda8b2b58cd834e35c17ecd1610790fdc72179876b861b6f` |
| `runs\<stamp>\report.json`, `inputs.jsonl` | one folder per run; every input sent is in its `inputs.jsonl` (ADR 0010 Amendment 2's record) |

The game was kept up by session `2026-11-16-T06`, ledger entry `game-20261116-06`
(`docs/game-observations.md`; its probe rows are not input evidence). Nothing here is a
capability status of a record type: it grades the executor, not a plugin.

## Findings

| What | Focused (game in front) | Unfocused |
| ---- | ----------------------- | --------- |
| Keyboard (`SendInput` scan codes) | **works**: W for 2 s moved 6.68 units (3.34 units/s); E, R, Tab act | no effect (0.00 units) |
| Mouse look (relative moves) | **works**: +400 counts X turned +45.4 degrees, **0.1134 degrees per count**; Y pitches the camera (about 0.085 degrees per count) but `getangle x` stays 0 and `setangle x` does not move the first-person camera | no effect (0.0 degrees) |
| Mouse buttons | **work**: three left clicks fired the Eon, ammo 129 to 126 | not tried |
| Built-in virtual pad (`InputInjector`, no driver) | **invisible to the game**: injection initialises and runs, `XInputGetState` for pads 0 to 3 answers 1167 (not connected), the player does not move | the same |

The guard (input only while the game window is in front) is therefore not only a safety
rule: the game ignores input it does not have focus for.

## Scenarios

| Id | Result | Note |
| -- | ------ | ---- |
| SC-1 container, take all | **pass** (third try) | `player.getitemcount 0000000F` 738 to 4268; E at a container is quick-loot (takes the highlighted item), R opens Transfer, R inside it is Take All |
| SC-3 equip and fire | **pass** (second try) | the Eon's ammo is `04AD3E` (from `kit inspect runtime:000476C4`), not the form first assumed |
| SC-2 NPC dialogue, first option | **partial** | `player.moveto 0000598D` (NoelREF) landed in a quest scene the base save started by itself; E chose the first option and the dialogue advanced. Opening dialogue **with E** was not isolated |
| SC-4 workbench | **inconclusive** | the "Weapon Workbench / Use (E)" prompt showed; E hid the HUD briefly and returned to the world, no menu |
| SC-5 menu save | skipped | no permission for a save not named `KitKraftSession-*` (bench sheet A4) |

### Second night (2026-11-17, 00:42 to 00:46, unattended)

Inside session `2026-11-17-T04` (ledger `game-20261117-04`, which holds the details), the
game kept up by `game.session` `start: true` through a long-lived `kit mcp` of the release
`kit.exe`. Run without `--ask`; the agent read the screenshots. Every input sent is in each
run's `inputs.jsonl` under `C:\Modding\kitkraft-observe\scripts\input\runs\`.

| Id | Result | Note |
| -- | ------ | ---- |
| SC-4 workbench | **pass** (retry) | the scenario's aim (10 degrees down) left the crosshair above the bench (`20261117-004233-scenarios`); 300 more counts down showed the bench's name, E opened the workbench menu, Tab left it (`20261117-adhoc-sc4`). The scenario's pitch should go about 35 degrees down from level |
| SC-2 dialogue opened by E | **pass** | `npc_ref=000057BE` (Vasco, Lodge): "Talk (E)", E opened the dialogue, E picked the first option and the line played (`20261117-004401-scenarios`). The scenario's closing Tab, sent during the NPC's line, did nothing; Tab at the option list closed the dialogue (`20261117-adhoc-sc2`) |
| SC-5 menu save | **pass** | allowed since ADR 0010 Amendment 3; the save landed in the dev profile's `saves\` (`--param save_dir=` must name that folder: the file's default points at the owner's `Documents` saves, which it only lists); after the menu load x was back at -12.99 (`20261117-004532-scenarios`) |

Also learned: the scenarios' assumed pause-menu order (Resume, Save, Load) is wrong in
1.16.244 (the menu reads Quicksave, Save, Load, ...), yet one Down plus E reached Save, so
the cursor starts on Quicksave; and the load list showed only the new save, newest first.

Also learned: Tab in the world opens the Data menu, and **opening it made the game
autosave** into the dev profile's saves (bench sheet, notes); R with nothing targeted
reloads; R in the Data menu is Set Course. A key whose meaning depends on the screen needs
the screen checked first.

## Measurements (`runs\20261116-041025-measure\report.json`)

| What | Result |
| ---- | ------ |
| Console round trip (`player.getpos x`) | median **122 ms** (106 to 137, n = 20), 20 of 20 answered on the first send; `getav` 115 ms; the console also answered while the quick-loot panel was up (110 ms) |
| Screenshot of the game window, 1024 px PNG | median **133 ms** (129 to 144, n = 10): capture about 50 ms, PNG encoding the rest |
| Screenshot, full 1920 px PNG | median 360 ms (343 to 368, n = 5) |
| `face` by `setangle` | error 0.00 degrees in 3 of 3 (one read each) |
| `face` by mouse (0.1134 degrees per count) | error 0.04, 0.05, 0.02 degrees (two reads, 0.5 to 1.3 s) |
| `walk_to` (tolerance 0.6 units) | final distance 0.58, 2.62, 0.43 units; the second blocked by placed objects. The first version overshot (3.3 units/s against a poll cycle of about 0.6 s); fixed by releasing one cycle early and finishing with timed pulses |

## Recommendation (design input for ADR 0010 Amendment 2's executor)

- **A small `SendInput` executor with a foreground guard**: scan-code keys, relative mouse
  moves and mouse buttons, sent only while the game window is in front, every key released
  on stop. It is small enough to live in `crates/game` behind an `Input` trait, with every
  input recorded in the session's report (KK-362 builds that record).
- **A screen check before screen-dependent keys** (Tab, R, E): a 1024 px screenshot costs
  about 133 ms.
- **No pad.** Nothing tried needs one (walking, looking, firing, looting, dialogue), and the
  built-in virtual pad never reaches the game; a working pad would need a driver install,
  which is the owner's call and is not proposed.
- Which design to adopt stays the owner's pick (Amendment 2: nothing is launched for input
  until a design is chosen; that night was the trial).
