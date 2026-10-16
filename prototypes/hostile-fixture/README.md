# Hostile-encounter fixture (prototype, M5.H2 first half)

Purpose: the generic hostile-encounter fixture that the M5.H1 research note proposes
(KitKraft repo, `docs/research/hostile-encounters-2026-10-16.md`, section 4), built with
what KitKraft writes today. It is a test fixture for KitKraft's authoring and probe
paths, **not** a Foothold attack system and not a mod to play with.

- Tickets: M5.H2 (first half: configurable trigger, actors, spawn location and cooldown,
  deterministic test trigger, cancel and cleanup). The second half (combat behaviour,
  defenses, damage and recovery, attended versus unattended, vanilla and DLC
  compatibility) is not started.
- Last built with KitKraft at `97dc5ee` (plus this lane's commits), 2026-10-16.
- Files: `hostile-fixture.toml` (build v3 spec), `scripts/KitKraft/HostileFixtureController.psc`
  (ours). Nothing else; output goes to `out/prototypes/hostile-fixture/`.

## What it is

| Part | Record | How it is written |
| ---- | ------ | ----------------- |
| Quest `KitKraftHostileFixtureQuest` | `QUST`, two stages (10 and 20), no fragments, aliases, objectives or log entries | `[[new]]` with `INDX.StageIndex` = 10 and `INDX[1].StageIndex` = 20; `DNAM`, `NEXT` and `ANAM` zeros. The shape of vanilla `City_NA_Landing_ShipTechs` (`Starfield.esm:252335`), whose stages are bare `INDX` too |
| Telemetry `KitKraftHostileFixtureSpawned` | `GLOB` 0 | `[[new]]`, `FLTV` 0 |
| Cooldown `KitKraftHostileFixtureNextDay` | `GLOB` 0 (game days) | `[[new]]`, `FLTV` 0: the first trigger always fires |
| Fallback trigger `KitKraftHostileFixtureTrigger` | `GLOB` 0 | `[[new]]`: 1 spawns, 2 cleans up (the research's G1 fallback, kept beside the quest) |
| Controller `KitKraftHostileFixtureController` | persistent `REFR` of `ActivatorSwitch01` (`Starfield.esm:03CCF4`) at (-15, -4, 4.63) in `CityNewAtlantisApartmentTEMPLATE` (`Starfield.esm:21966F`, the first POI) | `[[place]]` with `scripts`: `KitKraft:HostileFixtureController` and its ten properties, the four object ones naming the forms above and the actor base |
| Actors | `LvlStarborn_Aggro` (`NPC_` `Starfield.esm:27025D`), whose template `TPLT` is `LCharStarborn` (`LVLN` `Starfield.esm:270261`) | `SpawnBase` property; `PlaceActorAtMe` takes an `ActorBase`, so the leveled list is reached through the templated `NPC_` |

The script, as behaviour: on load, register for the quest's remote `OnStageSet` and poll
the trigger global once a second; stage 10 (or trigger 1) refuses while the game time is
below the cooldown global, else places `SpawnCount` (2) actors at the controller, keeps
them, sets the telemetry global to the count and the cooldown to now plus
`CooldownDays` (0); stage 20 (or trigger 2) disables and deletes every kept actor and
sets the telemetry to 0; on unload it cancels the poll and touches nothing. Every step
is traced (`KitKraftHostileFixture loaded | spawn <n> | cooldown ... | cleanup <n>`).
Declarations used (`kit papyrus events`): `Quest.OnStageSet` (`Quest.psc:294`),
`ObjectReference.PlaceActorAtMe` (863), `ScriptObject.RegisterForRemoteEvent` (196),
`Utility.GetCurrentGameTime` (21), `Debug.Trace` (100).

## Build and check

```text
kit papyrus compile prototypes/hostile-fixture/scripts/KitKraft/HostileFixtureController.psc out/prototypes/hostile-fixture/Scripts --sources-zip "<Starfield>/Tools/ContentResources.zip"
kit build prototypes/hostile-fixture/hostile-fixture.toml out/prototypes/hostile-fixture/KitKraftHostileFixture.esm
kit validate --plugin out/prototypes/hostile-fixture/KitKraftHostileFixture.esm
kit roundtrip out/prototypes/hostile-fixture/KitKraftHostileFixture.esm --verify
kit refs KitKraftHostileFixtureController --plugins Starfield.esm --plugins out/prototypes/hostile-fixture/KitKraftHostileFixture.esm
kit testplan out/prototypes/hostile-fixture/KitKraftHostileFixture.esm out/prototypes/hostile-fixture/test --plugins Starfield.esm --spec prototypes/hostile-fixture/hostile-fixture.toml
```

2026-10-16 (release build): `KitKraftHostileFixture.esm` 2,479 bytes, SHA-256
`614beeaa7f368be27266b606cc4ab68009df1a3a8eaf93ca3d476c095fb26107`, one master
(`Starfield.esm`), new forms `:001000` (`QUST`), `:001001` to `:001003` (`GLOB`),
`:001004` (`REFR`); `validate` 0 errors, 0 warnings (`papyrus.missing-script` and
`papyrus.property-unresolved` included: all ten properties declared with matching
types); `roundtrip --verify` identical; `refs` of the controller: six links, all resolved
(the base and the five object properties). `HostileFixtureController.pex` 4,124 bytes,
SHA-256 `b3a81cbb185bc65a00c643fe0329a5e887e2e6c8fd7db68d2b0ede1bf40ee78e`
(PapyrusCompiler 4.7.0.5). Spec SHA-256
`a7bb87c033287141e287f43ab720e15ac8f64b349fb7b54f84ad1e38de68c115`. `testplan`: 10
probes (a `help` probe per form and `getstage` on the quest, the cell, the controller's
position and `isdisabled`, the persistence protocol).

## Probes `testplan` does not emit yet (to run by hand or add to M4.T2)

`testplan` derives probes from records; it does not know what a script does. The
behaviour rows of the research note (H2-03 to H2-11), adapted to what was built:

| Row | Command(s) | Matcher | Expected |
| --- | ---------- | ------- | -------- |
| H2-03 | `getstage KitKraftHostileFixtureQuest` | number | 0 |
| H2-04 | `getglobalvalue KitKraftHostileFixtureSpawned` | number | 0 |
| H2-05 | `setstage KitKraftHostileFixtureQuest 10`; wait 2 s; `getglobalvalue KitKraftHostileFixtureSpawned` | number | 2 |
| H2-05b | (fallback path) `set KitKraftHostileFixtureTrigger to 1`; wait 2 s; `getglobalvalue KitKraftHostileFixtureSpawned` | number | 2 |
| H2-06 | `Papyrus.0.log` | papyrus-log contains | `KitKraftHostileFixture spawn 2` |
| H2-07 | screenshot | manual | two Starborn hostile to the player (no navmesh in this cell: they may not move) |
| H2-08 | `setstage KitKraftHostileFixtureQuest 20`; `getglobalvalue KitKraftHostileFixtureSpawned` | number, plus papyrus-log contains `cleanup 2` | 0 |
| H2-09 | `set KitKraftHostileFixtureNextDay to <GameDaysPassed + 1>`; `set KitKraftHostileFixtureTrigger to 1`; wait 2 s; `getglobalvalue KitKraftHostileFixtureSpawned` | number, plus papyrus-log contains `cooldown` | 0 |
| H2-10 | after a spawn: `save KitKraftSession-<date>-H2`; `load` it; `getglobalvalue KitKraftHostileFixtureSpawned`; `getstage KitKraftHostileFixtureQuest` | number | 2 and 10 (whether placed actors persist is PS-09) |
| H2-11 | after a spawn: leave the cell, pass an hour, return; `help KitKraftFirstPOICrate 4` with the POI plugin loaded | contains, plus manual | nothing destroyed while away |

## What could not be authored, and what blocks the full fixture

- **Authored, but unproven:** the quest. `new QUST` writes two bare stages, and the
  plugin validates and round-trips, but no test in the repo covers writing a `QUST`
  (gap matrix: typed-writable `untested`), its `DNAM` is zeros (not start-game-enabled,
  priority 0) and each `INDX`'s flags and last byte are 0 where vanilla's are not. That
  `setstage` starts such a quest and fires a remote `OnStageSet` is grade D. The trigger
  global is the fallback that needs no quest at all.
- **Not authored:** fragments, aliases, objectives and stage log entries (none needed);
  a link from the controller to a location (authorable since lane D round 5, as
  `docs/recipes/mini-site.md` lists its references in its location's `LCPR`, but the
  fixture needs no location, so the plugin is unchanged); a navmesh (the cell has none).
- **Blockers for the full fixture:** the scripts recipe in game (owner row 14, O4-07, or
  M5.B3); lane C's live sessions (M4.T1, M4.O7) to run the probes; probe rows for
  scripted behaviour (the table above) in `testplan`; PS-09 (whether `PlaceActorAtMe`
  actors survive save and reload); M5.H2's second half.

## Player-agency constraints (verbatim)

From the research note's section 5, quoting the owner's addendum item 4:

Owner, addendum item 4, recorded as future Foothold design constraints: "Record as future
Foothold design constraints: optional/configurable attacks, meaningful defenses, reliable
notifications, nonpunitive unattended resolution, and no arbitrary loss of accumulated
player investment."

How this fixture keeps to them: the encounter fires only on an explicit trigger (stage or
global), never on its own; the cooldown is a global anyone can set; nothing runs while
the controller is unloaded; cleanup deletes only the actors the fixture placed.

## Hand-off status

`validation`: parse pass, byte_roundtrip pass, ck_open not-run, ck_reexport not-run,
game_runtime not-run, save_reload not-run; `result: partial`.
