# MEL DLC extension brief

Owner: Michael, 2026-10-18. Goal in one sentence: DLC enemies should never spawn below 75% of
the player's level, the same way Minimum Enemy Level (MEL, Nexus 9477, v1.2) already treats
the base game's enemies.

## Background (read off the plugins, 2026-10-18)

MEL is `Minimum Enemy Level.esm` (559 overrides: 556 `NPC_`, 3 `GMST`). Comparing every MEL
override with the version before it (`kit conflicts --plugin "Minimum Enemy Level.esm"`,
then `kit inspect` of each form on the Default profile) gives one rule for the level fields:

| Field | MEL's value | Records |
| :--- | :--- | :--- |
| `ACBS.Flags` | old flags plus `AutoCalcStats` (16) and `PcLevelMult` (128) | 556 of 556 |
| `ACBS.Level` (the multiplier, thousandths, once `PcLevelMult` is set) | 750; 1000 for Starborn and Terrormorph | 503 / 53 |
| `ACBS.CalcMinLevel` | the record's old fixed level | 545 of 556 (11 hand-picked) |
| `ACBS.CalcMaxLevel` | 0 (no cap) | 556 of 556 |

MEL's health edits follow no rule (Spacers and Va'ruun lowered, Marines, robots, turrets and
Terrormorphs untouched), so build 1 leaves health alone (B3).

None of the DLC's own enemy tier templates is in MEL: of 1,295 DLC `NPC_` records, only 24
(SFBGS00D Cataxi) inherit their level from a MEL record.

## Rows

| ID | What (owner's words) | Forms affected | Desired change | Measurable target |
| :- | :------------------- | :------------- | :------------- | :---------------- |
| B1 | Extend MEL's level treatment to the DLC enemies | The DLC's numbered enemy tier templates that hold their own level (no `Stats` template flag, no `PcLevelMult` yet): Terran Armada `SFTER_EncTerranHuman_{Grunt,Officer}NN`, `SFTER_EncTerranRobot_{Scout,Soldier,Devastator}NN`, `SFTER_EncTurretBallisticNN`; Shattered Space `SFBGS001_EncVortex{Horror,HorrorSmall,Phantom,Spawn}NN`, `SFBGS001_EncVaruunRedeemedNN`, `SFBGS001_EncVaruun00`, `SFBGS001_EncVaruunTurret01` | MEL's rule above, multiplier 0.75 for every family (Vortex Horrors included: owner, 2026-10-18) | `kit build --plan` shows on each record: Flags gains 0x90, Level = `EE02` (750), CalcMinLevel = old level, CalcMaxLevel = 0. In game, on the `Kitkraft Dev` profile at player level L: a Terran grunt or Vortex enemy whose template level is below 0.75·L reads at least floor(0.75·L) with `getlevel` |
| B2 | Bounty targets too (owner works the bounty board) | Trackers Alliance `Enc_SFBGS003_BountyTargetNPC_{City,NonCity}NN` | As B1 | As B1, on a Trackers Alliance bounty target |
| B3 | Health: measure before changing | Same forms | None in build 1. Build 2 decides from measurements | Owner records `getav health` on one Terran grunt and one Vortex Phantom with MEL enabled and disabled; build 2 is planned from those numbers |

## Build 1 status (2026-10-18)

- Two plugins, not one: a patch whose master list differs from the source plugin's must remap
  FormIDs, and `build` refuses that for `NPC_` records with opaque subrecords (`NAM5`). So
  `docs/mel-dlc/gen_spec.py` writes one spec per DLC:
  `mel-dlc-shattered-space.toml` (44 records, `KitKraftMELDLC_ShatteredSpace.esm`, SHA-256
  `5119946d…`) and `mel-dlc-terran-armada.toml` (135 records, `KitKraftMELDLC_TerranArmada.esm`,
  SHA-256 `11a337c9…`). Build, validate, roundtrip and package pass for both; deploy, the
  session and every in-game row are not-run.
- B2 is blocked: the bounty targets win in `SFBGS003.esm`, a medium-scale plugin, and `build`
  refuses to copy a scaled plugin's records until a scale-aware remap exists (lane D).
  `mel-dlc-trackers-alliance.toml` lists the 20 skipped forms and has no entries.
- `testplan` writes no console rows for override-only level edits; the in-game checklist below
  is the check.

## Out of scope

Civilians, guards, faces, corpses, ship pilots (`*_AutopilotAI`), audio-only placeholders,
named, unique or essential characters; the 16 vanilla tier records MEL skipped (mining
turrets, mini-bots, settlers); MEL's own records and its `GMST` edits; companions.

## In-game checklist

| Check | Brief row | Expected | Seen | Pass |
| :---- | :-------- | :------- | :--- | :--- |
| Terran Armada grunt in a low-level system | B1 | `getlevel` ≥ floor(0.75·L) | | |
| Vortex enemy (Shattered Space) | B1 | `getlevel` ≥ floor(0.75·L) | | |
| Trackers Alliance bounty target | B2 | `getlevel` ≥ floor(0.75·L) | | |
| Health with MEL on / off (grunt, Phantom) | B3 | recorded, no pass/fail | | |
